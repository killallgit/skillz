use std::path::Path;

use anyhow::Context;

use crate::env::Env;
use crate::model::Scope;
use crate::prompt::Prompter;
use crate::runner::CommandRunner;

use super::Cmd;
use super::source::run_checked;

/// A `.claude-plugin/marketplace.json` reduced to what install needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Marketplace {
    pub name: String,
    pub plugins: Vec<String>,
}

/// `None` when `src` has no marketplace; malformed JSON is an error.
pub(super) fn read(src: &Path) -> anyhow::Result<Option<Marketplace>> {
    let path = src.join(".claude-plugin/marketplace.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    let Some(name) = json["name"].as_str() else {
        return Ok(None);
    };
    let plugins = json["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["name"].as_str().map(String::from))
        .collect();
    Ok(Some(Marketplace {
        name: name.into(),
        plugins,
    }))
}

/// Let the user pick `mp`'s plugins, then add the marketplace to Claude and install them.
/// `Ok(false)` when nothing was picked.
pub(super) fn install(
    mp: &Marketplace,
    source: &str,
    scope: Scope,
    env: &Env,
    runner: &dyn CommandRunner,
    prompter: &dyn Prompter,
) -> anyhow::Result<bool> {
    let picked = prompter.multi_select("Plugins to install", &mp.plugins)?;
    if picked.is_empty() {
        return Ok(false);
    }
    let chosen: Vec<String> = picked.iter().map(|&i| mp.plugins[i].clone()).collect();
    for (program, args) in commands(source, &mp.name, &chosen, scope) {
        run_checked(runner, &program, &args, &env.project)?;
    }
    Ok(true)
}

fn commands(source: &str, marketplace: &str, plugins: &[String], scope: Scope) -> Vec<Cmd> {
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "project",
    };
    let claude = |args: &[&str]| {
        (
            "claude".to_string(),
            args.iter().map(|s| s.to_string()).collect(),
        )
    };
    std::iter::once(claude(&["plugin", "marketplace", "add", source]))
        .chain(plugins.iter().map(|p| {
            claude(&[
                "plugin",
                "install",
                &format!("{p}@{marketplace}"),
                "--scope",
                scope,
            ])
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::test_support::*;
    use std::fs;

    fn mp() -> Marketplace {
        Marketplace {
            name: "mp".into(),
            plugins: s(&["p1", "p2"]),
        }
    }

    #[test]
    fn read_parses_name_and_plugins() {
        let fx = project_with(marketplace_fixture);
        let found = read(&fx.env.project.join("repo")).unwrap();
        assert_eq!(found, Some(mp()));
        assert_eq!(read(&fx.env.project).unwrap(), None);
    }

    #[test]
    fn malformed_marketplace_is_err() {
        let fx = project_with(|root| {
            fs::create_dir_all(root.join(".claude-plugin")).unwrap();
            fs::write(root.join(".claude-plugin/marketplace.json"), "{nope").unwrap();
        });
        assert!(read(&fx.env.project.join("repo")).is_err());
    }

    #[test]
    fn adds_marketplace_and_installs_picked_plugins() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0, 1], true);
        let installed = install(&mp(), "o/r", Scope::Project, &fx.env, &runner, &prompter);
        assert!(installed.unwrap());
        assert_eq!(prompter.offered.borrow()[0], s(&["p1", "p2"]));
        let calls: Vec<Cmd> = runner.calls().into_iter().map(|(p, a, _)| (p, a)).collect();
        assert_eq!(
            calls,
            vec![
                ("claude".into(), s(&["plugin", "marketplace", "add", "o/r"])),
                (
                    "claude".into(),
                    s(&["plugin", "install", "p1@mp", "--scope", "project"])
                ),
                (
                    "claude".into(),
                    s(&["plugin", "install", "p2@mp", "--scope", "project"])
                ),
            ]
        );
    }

    #[test]
    fn cancel_installs_nothing() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], true);
        let installed = install(&mp(), "o/r", Scope::Project, &fx.env, &runner, &prompter);
        assert!(!installed.unwrap());
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn claude_plugin_failure_is_err() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {}).failing("claude");
        let prompter = ScriptedPrompter::new(&[0], true);
        assert!(install(&mp(), "o/r", Scope::Project, &fx.env, &runner, &prompter).is_err());
        assert_eq!(runner.calls().len(), 1);
    }

    #[test]
    fn commands_scope_flags() {
        let user = commands("o/r", "mp", &s(&["p"]), Scope::Global);
        assert_eq!(
            user[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "user"])
        );
        let proj = commands("o/r", "mp", &s(&["p"]), Scope::Project);
        assert_eq!(
            proj[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "project"])
        );
    }
}
