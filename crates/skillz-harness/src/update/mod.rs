//! Updating what is installed: the command of the tool that installed an item, per kind. What
//! has no such command (hooks, MCP servers, hand-made skills) has no update.

pub mod check;

use std::path::Path;

use crate::action::Action;
use crate::agents::{Harness, adapter_for};
use crate::model::{Details, Installed, Installer, Kind};
use crate::npx;

/// The steps that bring `item` to the latest version, run from `cwd`; none when it cannot be
/// updated. An item seen through several agents is updated through each of them.
pub fn steps(adapters: &[Box<dyn Harness>], item: &Installed, cwd: &Path) -> Vec<Action> {
    item.parts()
        .iter()
        .flat_map(|part| part_steps(adapters, part, cwd))
        .collect()
}

fn part_steps(adapters: &[Box<dyn Harness>], item: &Installed, cwd: &Path) -> Vec<Action> {
    let name = match &item.details {
        Details::Skill { install_name, .. } => install_name.as_str(),
        _ => item.name.as_str(),
    };
    if name.is_empty() || name.starts_with('-') {
        return vec![];
    }
    match (item.installer, item.kind) {
        (Installer::Npx, Kind::Skill) => vec![npx::plan_update(name, item.scope, cwd)],
        (Installer::Local, Kind::Subagent) => recopy(item, cwd),
        _ => item
            .agents
            .iter()
            .filter_map(|&agent| adapter_for(adapters, agent))
            .flat_map(|adapter| adapter.plan_update(item, cwd))
            .collect(),
    }
}

pub fn can_update(item: &Installed) -> bool {
    !steps(&crate::agents::registry(), item, Path::new("")).is_empty()
}

/// A subagent copied from a source is copied again from it.
fn recopy(item: &Installed, cwd: &Path) -> Vec<Action> {
    let Some(source) = item.source.clone() else {
        return vec![];
    };
    let Details::Subagent { file } = &item.details else {
        return vec![];
    };
    let (Some(to), Some(stem), Some(ext)) = (file.parent(), file.file_stem(), file.extension())
    else {
        return vec![];
    };
    vec![Action::CopyAgents {
        source,
        cwd: cwd.to_path_buf(),
        names: vec![stem.to_string_lossy().into_owned()],
        ext: ext.to_string_lossy().into_owned(),
        to: to.to_path_buf(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Agent;
    use crate::model::Scope;

    fn item(name: &str, kind: Kind, installer: Installer) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![if installer == Installer::CodexCli {
                Agent::Codex
            } else {
                Agent::Claude
            }],
            scope: if installer == Installer::CodexCli {
                Scope::Global
            } else {
                Scope::Project
            },
            group: "g".into(),
            source: None,
            installer,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn programs(item: &Installed) -> Vec<String> {
        steps(&crate::agents::registry(), item, Path::new("/p"))
            .iter()
            .map(|step| match step {
                Action::Run { program, .. } => program.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn each_tool_updates_what_it_installed() {
        let cases = [
            (Kind::Skill, Installer::Npx, vec!["npx"]),
            (Kind::Plugin, Installer::ClaudeCli, vec!["claude"]),
            (Kind::Marketplace, Installer::ClaudeCli, vec!["claude"]),
            (Kind::Marketplace, Installer::CodexCli, vec!["codex"]),
            (Kind::Plugin, Installer::CodexCli, vec!["codex", "codex"]),
        ];
        for (kind, installer, expected) in cases {
            assert_eq!(programs(&item("x@m", kind, installer)), expected);
        }
    }

    #[test]
    fn hooks_mcp_and_hand_made_skills_have_no_update() {
        for (kind, installer) in [
            (Kind::Hook, Installer::Local),
            (Kind::Mcp, Installer::Local),
            (Kind::Skill, Installer::Local),
        ] {
            assert!(!can_update(&item("x", kind, installer)));
        }
    }

    #[test]
    fn a_name_that_looks_like_an_option_is_never_passed_on() {
        assert!(!can_update(&item("-x", Kind::Skill, Installer::Npx)));
        for name in ["plugin@--help", "plugin@"] {
            assert!(!can_update(&item(name, Kind::Plugin, Installer::CodexCli)));
        }
    }

    #[test]
    fn a_skill_is_updated_at_the_scope_it_is_installed_in() {
        let mut global = item("tdd", Kind::Skill, Installer::Npx);
        global.scope = Scope::Global;
        let project = item("tdd", Kind::Skill, Installer::Npx);

        let flag =
            |item: &Installed| match &steps(&crate::agents::registry(), item, Path::new("/p"))[0] {
                Action::Run { args, .. } => args[4].clone(),
                other => panic!("{other:?}"),
            };

        assert_eq!(
            (flag(&global).as_str(), flag(&project).as_str()),
            ("-g", "-p")
        );
    }

    #[test]
    fn a_codex_plugin_refreshes_its_marketplace_before_it_is_added_again() {
        let plugin = item("p@mp", Kind::Plugin, Installer::CodexCli);

        let args: Vec<Vec<String>> = steps(&crate::agents::registry(), &plugin, Path::new("/p"))
            .into_iter()
            .map(|step| match step {
                Action::Run { args, .. } => args,
                other => panic!("{other:?}"),
            })
            .collect();

        assert_eq!(args[0][2], "upgrade");
        assert_eq!(args[0][3], "mp");
        assert_eq!(args[1][..2], ["plugin", "add"]);
    }

    #[test]
    fn a_subagent_with_a_source_is_copied_again_from_it() {
        let mut agent = item("reviewer", Kind::Subagent, Installer::Local);
        agent.source = Some("o/r".into());
        let dir = Path::new("/p/.claude/agents");
        agent.details = Details::Subagent {
            file: dir.join("reviewer.md"),
        };

        let planned = steps(&crate::agents::registry(), &agent, Path::new("/p"));

        assert!(matches!(
            &planned[..],
            [Action::CopyAgents { names, ext, to, .. }]
                if names == &["reviewer"] && ext == "md" && to == dir
        ));
        agent.source = None;
        assert!(!can_update(&agent));
    }
    #[test]
    fn a_renamed_skill_is_updated_by_its_installer_name() {
        let mut skill = item("my-link", Kind::Skill, Installer::Npx);
        skill.details = Details::Skill {
            directory: "/skills/real".into(),
            install_name: "real".into(),
        };

        let plan = steps(&crate::agents::registry(), &skill, Path::new("/project"));

        assert!(matches!(&plan[..], [Action::Run { args, .. }] if args[3] == "real"));
    }
    #[test]
    fn codex_project_plugins_have_no_native_update_command() {
        for kind in [Kind::Plugin, Kind::Marketplace] {
            let mut installed = item("plugin@marketplace", kind, Installer::CodexCli);
            installed.scope = Scope::Project;
            assert!(!can_update(&installed));
            installed.scope = Scope::Global;
            assert!(can_update(&installed));
        }
    }
}
