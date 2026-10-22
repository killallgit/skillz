//! Updating what is installed: the command of the tool that installed an item, per kind. What
//! has no such command (hooks, MCP servers, hand-made skills) has no update.

pub mod check;

use std::path::Path;

use crate::action::Action;
use crate::model::{Augmentation, Installer, Kind, Scope, split_plugin_id};

/// The steps that bring `item` to the latest version, run from `cwd`; none when it cannot be
/// updated. An item seen through several agents is updated through each of them.
pub fn steps(item: &Augmentation, cwd: &Path) -> Vec<Action> {
    item.parts()
        .iter()
        .flat_map(|part| part_steps(part, cwd))
        .collect()
}

fn part_steps(item: &Augmentation, cwd: &Path) -> Vec<Action> {
    let name = item.name.as_str();
    if name.is_empty() || name.starts_with('-') {
        return vec![];
    }
    let run = |program: &str, args: &[&str]| {
        let args = args.iter().map(|a| a.to_string()).collect();
        Action::required(program, args, cwd)
    };
    match (item.installer, item.kind) {
        (Installer::Npx, Kind::Skill) => vec![run("npx", &npx_args(name, item.scope))],
        (Installer::ClaudeCli, Kind::Plugin) => vec![run("claude", &["plugin", "update", name])],
        (Installer::ClaudeCli, Kind::Marketplace) => {
            vec![run("claude", &["plugin", "marketplace", "update", name])]
        }
        (Installer::CodexCli, Kind::Marketplace) => {
            vec![run("codex", &["plugin", "marketplace", "upgrade", name])]
        }
        (Installer::CodexCli, Kind::Plugin) => {
            let marketplace = split_plugin_id(name).1;
            vec![
                run("codex", &["plugin", "marketplace", "upgrade", marketplace]),
                run("codex", &["plugin", "add", name]),
            ]
        }
        (Installer::Local, Kind::Subagent) => recopy(item, cwd),
        _ => vec![],
    }
}

pub fn can_update(item: &Augmentation) -> bool {
    !steps(item, Path::new("")).is_empty()
}

fn npx_args(name: &str, scope: Scope) -> [&str; 6] {
    let scope = match scope {
        Scope::Global => "-g",
        Scope::Project => "-p",
    };
    ["-y", "skills", "update", name, scope, "-y"]
}

/// A subagent copied from a source is copied again from it.
fn recopy(item: &Augmentation, cwd: &Path) -> Vec<Action> {
    let Some(source) = item.source.clone() else {
        return vec![];
    };
    let file = item.removal.iter().find_map(|step| match step {
        Action::RemovePath(target) => Some(target.path()),
        _ => None,
    });
    let parts = file.and_then(|f| Some((f.parent()?, f.file_stem()?, f.extension()?)));
    let Some((to, stem, ext)) = parts else {
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
    use crate::action::RemovalTarget;
    use crate::model::Agent;

    fn item(name: &str, kind: Kind, installer: Installer) -> Augmentation {
        Augmentation {
            hooks: vec![],
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "g".into(),
            source: None,
            installer,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn programs(item: &Augmentation) -> Vec<String> {
        steps(item, Path::new("/p"))
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
    }

    #[test]
    fn a_skill_is_updated_at_the_scope_it_is_installed_in() {
        let mut global = item("tdd", Kind::Skill, Installer::Npx);
        global.scope = Scope::Global;
        let project = item("tdd", Kind::Skill, Installer::Npx);

        let flag = |item: &Augmentation| match &steps(item, Path::new("/p"))[0] {
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

        let args: Vec<Vec<String>> = steps(&plugin, Path::new("/p"))
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
        agent.removal = vec![Action::RemovePath(
            RemovalTarget::new(dir, ["reviewer.md"]).unwrap(),
        )];

        let planned = steps(&agent, Path::new("/p"));

        assert!(matches!(
            &planned[..],
            [Action::CopyAgents { names, ext, to, .. }]
                if names == &["reviewer"] && ext == "md" && to == dir
        ));
        agent.source = None;
        assert!(!can_update(&agent));
    }
}
