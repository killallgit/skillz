//! Planning a profile's entries into install steps.

use skillz_harness::Spec;
use skillz_harness::action::Action;
use skillz_harness::agents::{Harness, registry};
use skillz_harness::env::Env;
use skillz_harness::model::{Agent, Scope};
use skillz_harness::provision::{Provision, plan_entries};

use super::Profile;

fn plan(
    adapters: &[Box<dyn Harness>],
    profile: &Profile,
    scope: Scope,
    env: &Env,
) -> anyhow::Result<Provision> {
    profile.validate()?;
    Ok(plan_entries(adapters, &profile.resolved(), scope, env))
}

fn env() -> Env {
    Env::new("/h".into(), "/p".into())
}

fn profile(entries: Vec<Spec>) -> Profile {
    let mut p = Profile::new("t");
    p.entries = entries;
    p
}

fn skills(agents: Vec<Agent>) -> Spec {
    Spec::Skill {
        source: "o/r".into(),
        names: vec!["tdd".into()],
        agents,
    }
}

fn plugin(agents: Vec<Agent>) -> Spec {
    Spec::Plugin {
        source: "o/mp".into(),
        marketplace: "mp".into(),
        name: "pp".into(),
        agents,
    }
}

fn runs(steps: &[Action]) -> Vec<(&str, &[String])> {
    steps
        .iter()
        .map(|s| match s {
            Action::Run { program, args, .. } => (program.as_str(), args.as_slice()),
            other => panic!("not a command: {other:?}"),
        })
        .collect()
}

#[test]
fn skills_install_once_for_all_their_agents() {
    let p = plan(
        &registry(),
        &profile(vec![skills(vec![])]),
        Scope::Project,
        &env(),
    )
    .unwrap();
    let steps = runs(&p.steps);
    assert_eq!(steps.len(), 1);
    let args = steps[0].1;
    assert!(args.windows(2).any(|w| w == ["-a", "claude-code"]));
    assert!(args.windows(2).any(|w| w == ["-a", "codex"]));
    assert!(!args.contains(&"-g".to_string()));
    assert!(p.unsupported.is_empty());
}

#[test]
fn global_scope_adds_the_global_flag() {
    let p = plan(
        &registry(),
        &profile(vec![skills(vec![Agent::Codex])]),
        Scope::Global,
        &env(),
    )
    .unwrap();
    assert!(runs(&p.steps)[0].1.contains(&"-g".to_string()));
}

#[test]
fn plugins_go_to_each_agent_that_can_take_them() {
    let p = plan(
        &registry(),
        &profile(vec![plugin(vec![])]),
        Scope::Global,
        &env(),
    )
    .unwrap();
    let programs: Vec<&str> = runs(&p.steps).iter().map(|(prog, _)| *prog).collect();
    assert_eq!(programs, ["claude", "claude", "codex", "codex"]);
    assert!(p.unsupported.is_empty());
}

#[test]
fn project_plugins_are_supported_for_both_agents() {
    let p = plan(
        &registry(),
        &profile(vec![plugin(vec![])]),
        Scope::Project,
        &env(),
    )
    .unwrap();
    assert!(p.unsupported.is_empty());
    let programs: Vec<&str> = p
        .steps
        .iter()
        .filter_map(|step| match step {
            Action::Run { program, .. } => Some(program.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(programs, ["claude", "claude", "codex"]);
    assert!(p.steps.iter().any(|step| matches!(step, Action::Edit { file, .. } if file == &env().project.join(".codex/config.toml"))));
}

#[test]
fn project_plugins_install_to_local_scope_not_shared_settings() {
    let p = plan(
        &registry(),
        &profile(vec![plugin(vec![Agent::Claude])]),
        Scope::Project,
        &env(),
    )
    .unwrap();
    let install = runs(&p.steps)[1].1;
    assert_eq!(install, ["plugin", "install", "pp@mp", "--scope", "local"]);
}

fn mcp(command: Option<&str>, url: Option<&str>, transport: Option<&str>) -> Spec {
    Spec::Mcp {
        name: "srv".into(),
        command: command.map(String::from),
        args: vec!["-y".into(), "pkg".into()],
        url: url.map(String::from),
        transport: transport.map(String::from),
        agents: vec![],
    }
}

fn args_for(entry: Spec, agent: Agent, scope: Scope) -> Option<Vec<String>> {
    let mut p = profile(vec![entry]);
    p.agents = vec![agent];
    let planned = plan(&registry(), &p, scope, &env()).unwrap();
    planned.steps.first().map(|s| match s {
        Action::Run { args, .. } => args.clone(),
        other => panic!("{other:?}"),
    })
}

#[test]
fn mcp_servers_install_with_each_agents_own_command() {
    let stdio = || mcp(Some("npx"), None, None);
    assert_eq!(
        args_for(stdio(), Agent::Claude, Scope::Project).unwrap(),
        [
            "mcp", "add", "--scope", "local", "srv", "--", "npx", "-y", "pkg"
        ]
    );
    assert_eq!(
        args_for(stdio(), Agent::Claude, Scope::Global).unwrap()[3],
        "user"
    );
    assert_eq!(
        args_for(
            mcp(None, Some("https://x"), Some("sse")),
            Agent::Claude,
            Scope::Global
        )
        .unwrap(),
        [
            "mcp",
            "add",
            "--scope",
            "user",
            "--transport",
            "sse",
            "srv",
            "https://x"
        ]
    );
    assert_eq!(
        args_for(
            mcp(None, Some("https://x"), None),
            Agent::Claude,
            Scope::Global
        )
        .unwrap()[5],
        "http"
    );
    assert_eq!(
        args_for(stdio(), Agent::Codex, Scope::Global).unwrap(),
        ["mcp", "add", "srv", "--", "npx", "-y", "pkg"]
    );
    assert_eq!(
        args_for(
            mcp(None, Some("https://x"), None),
            Agent::Codex,
            Scope::Global
        )
        .unwrap(),
        ["mcp", "add", "srv", "--url", "https://x"]
    );
}

#[test]
fn mcp_servers_an_agent_cannot_take_are_reported() {
    assert!(args_for(mcp(Some("npx"), None, None), Agent::Codex, Scope::Project).is_none());
    assert!(
        args_for(
            mcp(None, Some("https://x"), Some("sse")),
            Agent::Codex,
            Scope::Global
        )
        .is_none()
    );
}

#[test]
fn subagents_copy_into_each_agents_own_directory() {
    let entry = || Spec::Subagent {
        source: "o/agents".into(),
        names: vec!["frontend".into()],
        agents: vec![],
    };
    let copy_to = |agent: Agent, scope: Scope| {
        let mut p = profile(vec![entry()]);
        p.agents = vec![agent];
        let planned = plan(&registry(), &p, scope, &env()).unwrap();
        planned.steps.first().map(|s| match s {
            Action::CopyAgents { to, ext, .. } => (to.clone(), ext.clone()),
            other => panic!("{other:?}"),
        })
    };
    assert_eq!(
        copy_to(Agent::Claude, Scope::Project),
        Some(("/p/.claude/agents".into(), "md".into()))
    );
    assert_eq!(
        copy_to(Agent::Claude, Scope::Global),
        Some(("/h/.claude/agents".into(), "md".into()))
    );
    assert_eq!(
        copy_to(Agent::Codex, Scope::Project),
        Some(("/p/.codex/agents".into(), "toml".into()))
    );
}

#[test]
fn an_invalid_profile_plans_nothing() {
    let mut p = profile(vec![skills(vec![])]);
    p.agents.clear();
    assert!(plan(&registry(), &p, Scope::Project, &env()).is_err());
}
