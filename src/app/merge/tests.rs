use super::*;
use skillz_harness::action::Action;
use skillz_harness::hook_text::HookEntry;
use skillz_harness::model::{Agent, Details, Installer, Kind, Scope};

fn item(kind: Kind, name: &str, agent: Agent) -> Installed {
    Installed {
        details: Default::default(),
        id: format!("{kind}:{agent}:{name}"),
        name: name.into(),
        kind,
        agents: vec![agent],
        scope: Scope::Global,
        group: "local".into(),
        source: None,
        installer: Installer::Local,
        origin: Default::default(),
        children: vec![],
        hooks: vec![],
        members: vec![],
        removal: vec![step(&format!("{agent}-{name}"))],
    }
}

fn step(name: &str) -> Action {
    Action::native("tool", &["remove", name], &[name], "/".as_ref()).unwrap()
}

fn hook(agent: Agent, command: &str) -> Installed {
    Installed {
        details: Default::default(),
        hooks: vec![HookEntry {
            event: "Stop".into(),
            matcher: None,
            command: command.into(),
        }],
        ..item(Kind::Hook, "rtk", agent)
    }
}

#[test]
fn a_skill_in_a_shared_dir_is_already_one_and_stays_unmerged() {
    let shared = Installed {
        agents: vec![Agent::Claude, Agent::Codex],
        ..item(Kind::Skill, "tdd", Agent::Codex)
    };

    let merged = merge(vec![shared.clone()]);

    assert_eq!(merged, [shared]);
}

#[test]
fn subagents_of_two_agents_become_one_with_both_agents_and_every_removal_step() {
    let claude = item(Kind::Subagent, "reviewer", Agent::Claude);
    let codex = item(Kind::Subagent, "reviewer", Agent::Codex);

    let merged = merge(vec![claude.clone(), codex.clone()]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].agents, [Agent::Claude, Agent::Codex]);
    assert_eq!(
        merged[0].removal,
        [claude.removal.clone(), codex.removal.clone()].concat()
    );
    assert_eq!(merged[0].members, [claude, codex]);
}

#[test]
fn the_merged_agents_are_in_agent_order_whatever_the_discovery_order() {
    let codex = item(Kind::Subagent, "reviewer", Agent::Codex);
    let claude = item(Kind::Subagent, "reviewer", Agent::Claude);

    let merged = merge(vec![codex, claude]);

    assert_eq!(merged[0].agents, [Agent::Claude, Agent::Codex]);
}

#[test]
fn a_hook_with_the_same_handler_in_two_agents_is_one() {
    let claude = hook(Agent::Claude, "rtk hook");
    let codex = hook(Agent::Codex, "rtk hook");

    let merged = merge(vec![claude, codex]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].hooks.len(), 1);
}

#[test]
fn hooks_of_one_program_are_one_row_whatever_each_agents_command_says() {
    let claude = hook(Agent::Claude, "rtk hook claude");
    let codex = hook(Agent::Codex, "rtk hook codex");

    let merged = merge(vec![claude, codex]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].agents, [Agent::Claude, Agent::Codex]);
    assert_eq!(merged[0].hooks.len(), 2);
    assert_eq!(merged[0].parts().len(), 2);
    assert_eq!(merged[0].removal.len(), 2);
}

#[test]
fn hooks_of_different_programs_stay_apart() {
    let claude = hook(Agent::Claude, "rtk hook");
    let codex = Installed {
        name: "bd".into(),
        ..hook(Agent::Codex, "bd hook")
    };

    assert_eq!(merge(vec![claude, codex]).len(), 2);
}

#[test]
fn the_same_name_in_different_scopes_is_not_merged() {
    let global = item(Kind::Subagent, "reviewer", Agent::Claude);
    let project = Installed {
        scope: Scope::Project,
        ..item(Kind::Subagent, "reviewer", Agent::Codex)
    };

    let merged = merge(vec![global, project]);

    assert_eq!(merged.len(), 2);
}

#[test]
fn the_same_name_from_different_sources_is_not_merged() {
    let from = |agent, source: &str| Installed {
        source: Some(source.into()),
        group: source.into(),
        ..item(Kind::Subagent, "reviewer", agent)
    };

    let merged = merge(vec![from(Agent::Claude, "a/x"), from(Agent::Codex, "b/y")]);

    assert_eq!(merged.len(), 2);
}

#[test]
fn mcp_servers_launched_differently_are_not_merged() {
    let server = |agent, launch: &str| Installed {
        details: Details::Mcp {
            config: "/config".into(),
            server: skillz_harness::mcp::Server {
                name: "github".into(),
                launch: skillz_harness::mcp::Launch::Stdio {
                    command: launch.into(),
                    args: vec![],
                },
                secrets: vec![],
            },
        },
        ..item(Kind::Mcp, "github", agent)
    };

    let merged = merge(vec![
        server(Agent::Claude, "npx a"),
        server(Agent::Codex, "npx b"),
    ]);

    assert_eq!(merged.len(), 2);
}

#[test]
fn plugins_merge_with_the_hooks_they_declare() {
    let plugin = |agent, command| Installed {
        details: Default::default(),
        hooks: hook(agent, command).hooks,
        group: "mp".into(),
        ..item(Kind::Plugin, "p@mp", agent)
    };

    let merged = merge(vec![plugin(Agent::Claude, "a"), plugin(Agent::Codex, "b")]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].hooks.len(), 2);
}

#[test]
fn merging_keeps_the_place_of_the_first_copy() {
    let items = vec![
        item(Kind::Skill, "first", Agent::Claude),
        item(Kind::Subagent, "r", Agent::Claude),
        item(Kind::Skill, "last", Agent::Claude),
        item(Kind::Subagent, "r", Agent::Codex),
    ];

    let merged = merge(items);

    let names: Vec<&str> = merged.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["first", "r", "last"]);
}

#[test]
fn parts_of_a_merged_item_are_its_copies_and_of_a_lone_item_itself() {
    let lone = item(Kind::Skill, "a", Agent::Claude);
    let merged = merge(vec![
        item(Kind::Subagent, "r", Agent::Claude),
        item(Kind::Subagent, "r", Agent::Codex),
    ]);

    assert_eq!(lone.parts(), std::slice::from_ref(&lone));
    assert_eq!(merged[0].parts().len(), 2);
}

#[test]
fn a_plugin_installed_for_two_agents_is_one_whatever_source_each_recorded() {
    let claude = Installed {
        source: Some("o/mp".into()),
        ..item(Kind::Plugin, "p@mp", Agent::Claude)
    };
    let codex = Installed {
        source: None,
        ..item(Kind::Plugin, "p@mp", Agent::Codex)
    };

    let merged = merge(vec![claude, codex]);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].agents, [Agent::Claude, Agent::Codex]);
    assert_eq!(merged[0].removal.len(), 2);
}
