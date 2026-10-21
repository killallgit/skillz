//! Turn a profile into the steps that install it. Pure: nothing runs here; `action::execute`
//! does. Skills go through `npx skills` for every agent at once; anything else is asked of each
//! agent's adapter (`AgentAdapter::provision`).

use crate::action::Action;
use crate::agents::{AgentAdapter, adapter_for};
use crate::env::Env;
use crate::model::{Agent, Scope};
use crate::npx_skills;
use crate::profile::{Entry, Profile};

/// An entry one agent has no way to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub entry: Entry,
    pub agent: Agent,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Provision {
    pub steps: Vec<Action>,
    pub unsupported: Vec<Unsupported>,
}

/// The steps that load `profile` into `scope`, in entry order.
pub fn plan(
    adapters: &[Box<dyn AgentAdapter>],
    profile: &Profile,
    scope: Scope,
    env: &Env,
) -> anyhow::Result<Provision> {
    profile.validate()?;
    Ok(plan_entries(adapters, &profile.resolved(), scope, env))
}

/// The steps that install `entries`, which already name the agents they are for.
pub fn plan_entries(
    adapters: &[Box<dyn AgentAdapter>],
    entries: &[Entry],
    scope: Scope,
    env: &Env,
) -> Provision {
    let mut out = Provision::default();
    for entry in entries {
        match skills_step(adapters, entry, scope, env) {
            Some(step) => out.steps.push(step),
            None => {
                let per_agent = provision_per_agent(adapters, entry, scope, env);
                out.steps.extend(per_agent.steps);
                out.unsupported.extend(per_agent.unsupported);
            }
        }
    }
    out
}

/// One `npx skills add` for all the entry's agents that this build knows; `None` unless the
/// entry is a skill.
fn skills_step(
    adapters: &[Box<dyn AgentAdapter>],
    entry: &Entry,
    scope: Scope,
    env: &Env,
) -> Option<Action> {
    let Entry::Skill { source, names, .. } = entry else {
        return None;
    };
    let known: Vec<Agent> = entry
        .agents()
        .iter()
        .copied()
        .filter(|a| adapters.iter().any(|ad| ad.agent() == *a))
        .collect();
    let (program, args) = npx_skills::add_command(source, names, &known, scope);
    Some(Action::required(&program, args, &env.project))
}

/// Ask each of the entry's agents for its own steps; the ones that cannot are reported.
fn provision_per_agent(
    adapters: &[Box<dyn AgentAdapter>],
    entry: &Entry,
    scope: Scope,
    env: &Env,
) -> Provision {
    let mut out = Provision::default();
    for &agent in entry.agents() {
        let steps = adapter_for(adapters, agent).and_then(|a| a.provision(entry, scope, env));
        match steps {
            Some(steps) => out.steps.extend(steps),
            None => out.unsupported.push(Unsupported {
                entry: entry.clone(),
                agent,
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry;

    fn env() -> Env {
        Env::new("/h".into(), "/p".into())
    }

    fn profile(entries: Vec<Entry>) -> Profile {
        let mut p = Profile::new("t");
        p.entries = entries;
        p
    }

    fn skills(agents: Vec<Agent>) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec!["tdd".into()],
            agents,
        }
    }

    fn plugin(agents: Vec<Agent>) -> Entry {
        Entry::Plugin {
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
    fn what_an_agent_cannot_take_is_reported_not_dropped() {
        // Codex plugin commands only exist for the global config.
        let p = plan(
            &registry(),
            &profile(vec![plugin(vec![])]),
            Scope::Project,
            &env(),
        )
        .unwrap();
        assert!(runs(&p.steps).iter().all(|(prog, _)| *prog == "claude"));
        assert_eq!(
            p.unsupported,
            [Unsupported {
                entry: plugin(vec![Agent::Claude, Agent::Codex]),
                agent: Agent::Codex
            }]
        );
        let cursor = plan(
            &registry(),
            &profile(vec![plugin(vec![Agent::Cursor])]),
            Scope::Global,
            &env(),
        )
        .unwrap();
        assert!(cursor.steps.is_empty());
        assert_eq!(cursor.unsupported.len(), 1);
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

    fn mcp(command: Option<&str>, url: Option<&str>, transport: Option<&str>) -> Entry {
        Entry::Mcp {
            name: "srv".into(),
            command: command.map(String::from),
            args: vec!["-y".into(), "pkg".into()],
            url: url.map(String::from),
            transport: transport.map(String::from),
            agents: vec![],
        }
    }

    fn args_for(entry: Entry, agent: Agent, scope: Scope) -> Option<Vec<String>> {
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
        assert!(args_for(mcp(Some("npx"), None, None), Agent::Cursor, Scope::Global).is_none());
    }

    #[test]
    fn subagents_copy_into_each_agents_own_directory() {
        let entry = || Entry::Subagent {
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
        assert_eq!(copy_to(Agent::Cursor, Scope::Global), None);
    }

    #[test]
    fn an_invalid_profile_plans_nothing() {
        let mut p = profile(vec![skills(vec![])]);
        p.agents.clear();
        assert!(plan(&registry(), &p, Scope::Project, &env()).is_err());
    }
}
