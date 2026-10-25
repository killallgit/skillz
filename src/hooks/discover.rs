use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

use super::group::{GroupHints, hook_group};
use crate::harness::action::{Action, Edit};
use crate::harness::read_optional::read_optional;
use crate::hook_text::{Format, HookEntry, parse};
use crate::model::{Agent, Installed, Installer, Kind, Scope};

/// Group of hooks that no plugin declares.
pub const UNOWNED_HOOKS: &str = "(unknown) hooks";

/// The config file a hook was read from, and whose it is.
struct Source<'a> {
    file: &'a Path,
    agent: Agent,
    scope: Scope,
}

/// Hooks in one config file, one `Installed` per `hook_group`, each with its removal plan.
/// A missing file yields an empty list.
pub fn discover(
    file: &Path,
    agent: Agent,
    scope: Scope,
    hints: &GroupHints,
) -> anyhow::Result<Vec<Installed>> {
    let Some(text) = read_optional(file)? else {
        return Ok(Vec::new());
    };
    let entries =
        parse(&text, Format::of(file)).with_context(|| format!("parse {}", file.display()))?;
    let source = Source { file, agent, scope };
    Ok(group_entries(entries, hints)
        .into_iter()
        .map(|(name, entries)| installed_item(&source, name, &entries))
        .collect())
}

fn group_entries(entries: Vec<HookEntry>, hints: &GroupHints) -> BTreeMap<String, Vec<HookEntry>> {
    let mut groups: BTreeMap<String, Vec<HookEntry>> = BTreeMap::new();
    for entry in entries {
        groups
            .entry(hook_group(&entry.command, hints))
            .or_default()
            .push(entry);
    }
    groups
}

fn installed_item(source: &Source, name: String, entries: &[HookEntry]) -> Installed {
    Installed {
        details: crate::model::Details::Hook {
            config: source.file.to_path_buf(),
        },
        hooks: entries.to_vec(),
        members: Vec::new(),
        id: format!("hook:{}:{}", source.file.display(), name),
        kind: Kind::Hook,
        agents: vec![source.agent],
        scope: source.scope,
        group: UNOWNED_HOOKS.into(),
        source: None,
        installer: Installer::Local,
        origin: Default::default(),
        children: entries.iter().map(HookEntry::label).collect(),
        removal: removal_plan(source.file, entries),
        name,
    }
}

fn removal_plan(file: &Path, entries: &[HookEntry]) -> Vec<Action> {
    let mut removal: Vec<Action> = Vec::new();
    for e in entries {
        let action = Action::Edit {
            file: file.to_path_buf(),
            edit: Edit::RemoveHook {
                event: e.event.clone(),
                command: e.command.clone(),
            },
        };
        if !removal.contains(&action) {
            removal.push(action);
        }
    }
    removal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_text::testutil::*;
    use serde_json::{Value, json};

    fn discover(file: &Path, agent: Agent, scope: Scope) -> anyhow::Result<Vec<Installed>> {
        super::discover(file, agent, scope, &GroupHints::default())
    }

    #[test]
    fn discover_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let found = discover(&dir.path().join("none.json"), Agent::Claude, Scope::Global).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn discover_invalid_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, "{").unwrap();
        assert!(discover(&file, Agent::Claude, Scope::Global).is_err());
        assert!(discover(dir.path(), Agent::Claude, Scope::Global).is_err());
    }

    #[test]
    fn discover_groups_and_plans_removal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let doc = json!({"hooks": {
            "SessionStart": [{"hooks": [{"command": "bd prime"}, {"command": "rtk hook claude"}]}],
            "PreCompact": [{"hooks": [{"command": "bd prime"}]}],
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"command": "rtk hook claude"}]},
                {"matcher": "Edit", "hooks": [{"command": "rtk hook claude"}]}
            ]
        }});
        std::fs::write(&file, doc.to_string()).unwrap();

        let found = discover(&file, Agent::Codex, Scope::Project).unwrap();
        let names: Vec<_> = found.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["bd", "rtk"]);

        let rtk = &found[1];
        assert_eq!(rtk.id, format!("hook:{}:rtk", file.display()));
        assert_eq!(rtk.kind, Kind::Hook);
        assert_eq!(rtk.installer, Installer::Local);
        assert_eq!(rtk.agents, vec![Agent::Codex]);
        assert_eq!(rtk.scope, Scope::Project);
        assert_eq!(rtk.group, UNOWNED_HOOKS);
        assert_eq!(rtk.source, None);
        assert_eq!(rtk.children.len(), 3);
        assert_eq!(rtk.hooks.len(), 3);
        let remove = |event: &str| Action::Edit {
            file: file.clone(),
            edit: Edit::RemoveHook {
                event: event.into(),
                command: "rtk hook claude".into(),
            },
        };
        assert_eq!(
            rtk.removal,
            vec![remove("SessionStart"), remove("PreToolUse")]
        );
        assert_eq!(found[0].removal.len(), 2);
    }

    #[test]
    fn discover_removal_empties_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hooks.json");
        let doc = json!({"version": 1, "hooks": {
            "sessionStart": [{"command": "bd cursor-hook sessionStart"}],
            "stop": [{"command": "/x/.orca/h.sh stop"}, {"command": "bd cursor-hook stop"}]
        }});
        std::fs::write(&file, doc.to_string()).unwrap();
        let plan: Vec<Action> = discover(&file, Agent::Claude, Scope::Global)
            .unwrap()
            .into_iter()
            .flat_map(|a| a.removal)
            .collect();
        crate::harness::action::execute(&plan, &crate::harness::runner::NoCommands).unwrap();
        let after: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(after, json!({"version": 1, "hooks": {}}));
    }

    #[test]
    fn discover_toml_plans_removal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, CODEX_TOML).unwrap();
        let found = discover(&file, Agent::Codex, Scope::Global).unwrap();
        let names: Vec<_> = found.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["bd", "rtk"]);
        let remove = |event: &str, command: &str| Action::Edit {
            file: file.clone(),
            edit: Edit::RemoveHook {
                event: event.into(),
                command: command.into(),
            },
        };
        assert_eq!(
            found[0].removal,
            vec![
                remove("SessionStart", "bd codex-hook SessionStart"),
                remove("Stop", "bd codex-hook Stop"),
            ]
        );
        assert_eq!(found[1].removal, vec![remove("Stop", "rtk hook codex")]);
    }
}
