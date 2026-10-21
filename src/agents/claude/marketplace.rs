//! Claude marketplaces: discovering the ones installed, and installing a source's plugins.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context;

use super::files::{EXTRA_KNOWN_MARKETPLACES, marketplaces_file};
use super::records::{Install, Marketplace, mp_of};
use super::removal::{plugin_removal, unique_in_order};
use crate::action::{Action, Edit, RemovalTarget};
use crate::agents::{PluginInstall, PluginOutcome};
use crate::install::{Cmd, run_checked};
use crate::model::{Agent, Augmentation, Installer, Kind, Scope};

/// One augmentation for an installed marketplace, removing it with all its plugins.
pub(super) fn discover(claude: &Path, m: &Marketplace, installs: &[Install]) -> Augmentation {
    let own: Vec<&Install> = installs.iter().filter(|i| mp_of(&i.id) == m.name).collect();
    Augmentation {
        id: format!("marketplace:claude:{}", m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Claude],
        scope: Scope::Global,
        group: m.name.clone(),
        source: m.source.clone(),
        installer: Installer::ClaudeCli,
        children: plugin_names(&own),
        removal: marketplace_removal(claude, m, &own),
    }
}

fn plugin_names(installs: &[&Install]) -> Vec<String> {
    let names: BTreeSet<String> = installs
        .iter()
        .map(|i| i.id.rsplit_once('@').map_or(&*i.id, |(n, _)| n).to_string())
        .collect();
    names.into_iter().collect()
}

fn marketplace_removal(claude: &Path, m: &Marketplace, own: &[&Install]) -> Vec<Action> {
    let mut removal = unique_in_order(own.iter().flat_map(|i| plugin_removal(claude, i)));
    let args = ["plugin", "marketplace", "remove", &m.name];
    removal.extend(Action::native("claude", &args, &[&m.name], claude));
    removal.push(Action::Edit {
        file: marketplaces_file(claude),
        edit: Edit::JsonRemoveKey {
            path: vec![m.name.clone()],
        },
    });
    removal.push(Action::Edit {
        file: claude.join("settings.json"),
        edit: Edit::JsonRemoveKey {
            path: vec![EXTRA_KNOWN_MARKETPLACES.into(), m.name.clone()],
        },
    });
    removal.extend(owned_clone(claude, m).map(Action::RemovePath));
    removal
}

/// Only a clone Claude made, as one segment under marketplaces/; a `directory` source's
/// location is the user's checkout.
fn owned_clone(claude: &Path, m: &Marketplace) -> Option<RemovalTarget> {
    let clones = claude.join("plugins/marketplaces");
    let rest = Path::new(&m.install_location).strip_prefix(&clones).ok()?;
    RemovalTarget::new(&clones, [rest])
}

/// A `.claude-plugin/marketplace.json` reduced to what install needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceMarketplace {
    name: String,
    plugins: Vec<String>,
}

/// An argument for `claude`, so never option-like.
fn cli_arg(v: &serde_json::Value) -> Option<&str> {
    v.as_str().filter(|s| !s.starts_with('-'))
}

/// `None` when `src` has no marketplace; malformed JSON is an error. Option-like (`-x`)
/// names are dropped since they end up as `claude` args.
fn read_source(src: &Path) -> anyhow::Result<Option<SourceMarketplace>> {
    let path = src.join(".claude-plugin/marketplace.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    let Some(name) = cli_arg(&json["name"]) else {
        return Ok(None);
    };
    let plugins = json["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| cli_arg(&p["name"]).map(String::from))
        .collect();
    Ok(Some(SourceMarketplace {
        name: name.into(),
        plugins,
    }))
}

/// Claude's side of `AgentAdapter::install_plugins`: only a source with a Claude marketplace.
pub(super) fn install_plugins(req: &PluginInstall) -> anyhow::Result<PluginOutcome> {
    let Some(mp) = read_source(req.src)? else {
        return Ok(PluginOutcome::Unsupported);
    };
    Ok(if install_picked(&mp, req)? {
        PluginOutcome::Installed
    } else {
        PluginOutcome::Declined
    })
}

/// Let the user pick `mp`'s plugins, then add the marketplace to Claude and install them.
/// `Ok(false)` when nothing was picked.
fn install_picked(mp: &SourceMarketplace, req: &PluginInstall) -> anyhow::Result<bool> {
    let picked = req
        .prompter
        .multi_select("Plugins to install", &mp.plugins)?;
    if picked.is_empty() {
        return Ok(false);
    }
    let chosen: Vec<String> = picked.iter().map(|&i| mp.plugins[i].clone()).collect();
    let scope = match req.scope {
        Scope::Global => "user",
        Scope::Project => "project",
    };
    for (program, args) in install_commands(req.source, &mp.name, &chosen, scope) {
        run_checked(req.runner, &program, &args, &req.env.project)?;
    }
    Ok(true)
}

/// `claude plugin marketplace add` then `plugin install` for each plugin; `scope` is the
/// `--scope` value (user, project or local).
fn install_commands(source: &str, marketplace: &str, plugins: &[String], scope: &str) -> Vec<Cmd> {
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

/// A profile's plugin for `scope`. A project's plugins go in the `local` scope, so they never
/// touch the settings file the team shares.
pub(super) fn provision(
    source: &str,
    marketplace: &str,
    plugin: &str,
    scope: Scope,
    cwd: &Path,
) -> Vec<Action> {
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "local",
    };
    install_commands(source, marketplace, &[plugin.to_string()], scope)
        .into_iter()
        .map(|(program, args)| Action::required(&program, args, cwd))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::Claude;
    use super::super::files::installed_file;
    use super::super::testutil::{by_id, dirs, files, fixture, remove_install, remove_key, rm};
    use super::*;
    use crate::agents::AgentAdapter;
    use crate::install::test_support::{
        FakeRunner, Fixture as ProjectFixture, ScriptedPrompter, marketplace_fixture, project_with,
        s,
    };
    use std::fs;
    use std::path::PathBuf;

    fn mp() -> SourceMarketplace {
        SourceMarketplace {
            name: "mp".into(),
            plugins: s(&["p1", "p2"]),
        }
    }

    fn install_picks(
        fx: &ProjectFixture,
        runner: &FakeRunner,
        prompter: &ScriptedPrompter,
    ) -> anyhow::Result<bool> {
        let req = PluginInstall {
            src: &fx.env.project,
            source: "o/r",
            scope: Scope::Project,
            env: &fx.env,
            runner,
            prompter,
        };
        install_picked(&mp(), &req)
    }

    #[test]
    fn marketplaces() {
        let f = fixture();
        let items = Claude.discover(&f.env, Scope::Global).unwrap();
        let home = &f.env.home;
        let installed = installed_file(&home.join(".claude"));
        let known = marketplaces_file(&home.join(".claude"));
        let global_settings = home.join(".claude/settings.json");

        let acme = by_id(&items, "marketplace:claude:acme");
        assert_eq!(acme.kind, Kind::Marketplace);
        assert_eq!(acme.installer, Installer::ClaudeCli);
        assert_eq!(acme.name, "acme");
        assert_eq!(acme.group, "acme");
        assert_eq!(acme.source.as_deref(), Some("acme/plugins"));
        assert_eq!(acme.children, ["solo", "tools"]);
        let p = &f.env.project;
        let other = f.env.project.parent().unwrap().join("other");
        assert_eq!(
            files(&acme.removal),
            [
                vec![
                    remove_key(&global_settings, &["enabledPlugins", "tools@acme"]),
                    remove_install(&installed, "tools@acme", None),
                ],
                dirs(home, "tools", "acme"),
                vec![
                    remove_key(
                        &p.join(".claude/settings.json"),
                        &["enabledPlugins", "tools@acme"]
                    ),
                    remove_key(
                        &p.join(".claude/settings.local.json"),
                        &["enabledPlugins", "tools@acme"]
                    ),
                    remove_install(&installed, "tools@acme", Some(p)),
                    remove_key(
                        &other.join(".claude/settings.json"),
                        &["enabledPlugins", "solo@acme"]
                    ),
                    remove_key(
                        &other.join(".claude/settings.local.json"),
                        &["enabledPlugins", "solo@acme"]
                    ),
                    remove_install(&installed, "solo@acme", Some(&other)),
                ],
                dirs(home, "solo", "acme"),
                vec![
                    remove_key(&known, &["acme"]),
                    remove_key(&global_settings, &["extraKnownMarketplaces", "acme"]),
                    rm(&home.join(".claude/plugins/marketplaces"), &["acme"]),
                ],
            ]
            .concat()
        );

        let other_mp = by_id(&items, "marketplace:claude:other");
        assert_eq!(other_mp.children, ["lint"]);
        assert_eq!(
            files(&other_mp.removal),
            [
                vec![
                    remove_key(&global_settings, &["enabledPlugins", "lint@other"]),
                    remove_install(&installed, "lint@other", None),
                ],
                dirs(home, "lint", "other"),
                vec![
                    remove_key(&known, &["other"]),
                    remove_key(&global_settings, &["extraKnownMarketplaces", "other"]),
                ],
            ]
            .concat()
        );
    }

    #[test]
    fn marketplace_keeps_locations_outside_its_dir() {
        let home = Path::new("/h");
        for loc in [
            "/Users/x/Code/repo",
            "/h/.claude/plugins/marketplaces",
            "/h/.claude/plugins/marketplaces/../../..",
            "/h/.claude/plugins/marketplaces/a/../../x",
            "/h/.claude/plugins/marketplaces/a/b",
            "",
        ] {
            let m = Marketplace {
                name: "local".into(),
                source: Some("/Users/x/Code/repo".into()),
                install_location: loc.into(),
            };

            let plan = discover(&home.join(".claude"), &m, &[]).removal;

            assert_eq!(
                files(&plan),
                [
                    remove_key(&marketplaces_file(&home.join(".claude")), &["local"]),
                    remove_key(
                        &home.join(".claude/settings.json"),
                        &["extraKnownMarketplaces", "local"]
                    ),
                ],
                "{loc}"
            );
        }
    }

    #[test]
    fn marketplace_removal_runs_claude_first() {
        let home = Path::new("/h");
        let user = Install {
            id: "p@mp".into(),
            project: None,
            scope: "user".into(),
            install_path: None,
        };
        let m = Marketplace {
            name: "mp".into(),
            source: None,
            install_location: String::new(),
        };

        let plan = discover(&home.join(".claude"), &m, &[user]).removal;

        let runs: Vec<&Action> = plan
            .iter()
            .filter(|a| matches!(a, Action::Run { .. }))
            .collect();
        assert_eq!(runs.len(), 2);
        let uninstall = Action::Run {
            program: "claude".into(),
            args: ["plugin", "uninstall", "p@mp", "--scope", "user"]
                .map(String::from)
                .to_vec(),
            cwd: PathBuf::from("/h/.claude"),
            fallback: true,
        };
        assert_eq!(*runs[0], uninstall);
        assert!(
            matches!(runs[1], Action::Run { args, .. } if args[..3] == ["plugin", "marketplace", "remove"])
        );
    }

    #[test]
    fn read_source_parses_name_and_plugins() {
        let fx = project_with(marketplace_fixture);

        let found = read_source(&fx.env.project.join("repo")).unwrap();

        assert_eq!(found, Some(mp()));
        assert_eq!(read_source(&fx.env.project).unwrap(), None);
    }

    #[test]
    fn malformed_marketplace_is_err() {
        let fx = project_with(|root| {
            fs::create_dir_all(root.join(".claude-plugin")).unwrap();
            fs::write(root.join(".claude-plugin/marketplace.json"), "{nope").unwrap();
        });

        let result = read_source(&fx.env.project.join("repo"));

        assert!(result.is_err());
    }

    #[test]
    fn read_source_skips_option_like_names() {
        let write = |json: &str| {
            project_with(move |root| {
                fs::create_dir_all(root.join(".claude-plugin")).unwrap();
                fs::write(root.join(".claude-plugin/marketplace.json"), json).unwrap();
            })
        };
        let fx = write(r#"{"name":"mp","plugins":[{"name":"-g"},{"name":"p1"},{"name":"p2"}]}"#);
        assert_eq!(
            read_source(&fx.env.project.join("repo")).unwrap(),
            Some(mp())
        );
        let fx = write(r#"{"name":"-mp","plugins":[{"name":"p1"}]}"#);
        assert_eq!(read_source(&fx.env.project.join("repo")).unwrap(), None);
    }

    #[test]
    fn adds_marketplace_and_installs_picked_plugins() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[0, 1], true);

        let installed = install_picks(&fx, &runner, &prompter);

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

        let installed = install_picks(&fx, &runner, &prompter);

        assert!(!installed.unwrap());
        assert!(runner.calls().is_empty());
    }

    #[test]
    fn claude_plugin_failure_is_err() {
        let fx = project_with(|_| {});
        let runner = FakeRunner::new(|_| {}).failing("claude");
        let prompter = ScriptedPrompter::new(&[0], true);

        let result = install_picks(&fx, &runner, &prompter);

        assert!(result.is_err());
        assert_eq!(runner.calls().len(), 1);
    }

    #[test]
    fn install_commands_scope_flags() {
        let user = install_commands("o/r", "mp", &s(&["p"]), "user");
        assert_eq!(
            user[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "user"])
        );
        let proj = install_commands("o/r", "mp", &s(&["p"]), "project");
        assert_eq!(
            proj[1].1,
            s(&["plugin", "install", "p@mp", "--scope", "project"])
        );
    }

    #[test]
    fn install_plugins_reports_unsupported_declined_and_installed() {
        let outcome = |with_mp: bool, picks: &[usize]| {
            let fx = project_with(move |root| {
                if with_mp {
                    marketplace_fixture(root)
                }
            });
            let runner = FakeRunner::new(|_| {});
            let prompter = ScriptedPrompter::new(picks, true);
            let src = fx.env.project.join("repo");
            install_plugins(&PluginInstall {
                src: &src,
                source: "o/r",
                scope: Scope::Project,
                env: &fx.env,
                runner: &runner,
                prompter: &prompter,
            })
            .unwrap()
        };
        assert_eq!(outcome(false, &[0]), PluginOutcome::Unsupported);
        assert_eq!(outcome(true, &[]), PluginOutcome::Declined);
        assert_eq!(outcome(true, &[0]), PluginOutcome::Installed);
    }
}
