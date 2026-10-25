//! Claude marketplaces: discovering the ones installed, and installing a source's plugins.

use std::collections::BTreeSet;
use std::path::Path;

use super::files::{EXTRA_KNOWN_MARKETPLACES, marketplaces_file};
use super::records::{Install, Marketplace};
use super::removal::{plugin_removal, unique_in_order};
use crate::agents::PluginOffer;
use crate::agents::plugin::read_marketplace;
use crate::harness::Spec;
use crate::harness::action::{Action, Edit, RemovalTarget};
use crate::harness::runner::Cmd;
use crate::model::{Agent, Installed, Installer, Kind, Scope, split_plugin_id};

/// One installed item for a marketplace, removing it with all its plugins.
pub(super) fn discover(claude: &Path, m: &Marketplace, installs: &[Install]) -> Installed {
    let own: Vec<&Install> = installs
        .iter()
        .filter(|i| split_plugin_id(&i.id).1 == m.name)
        .collect();
    Installed {
        details: crate::model::Details::Marketplace {
            location: m.install_location.clone().into(),
        },
        hooks: Vec::new(),
        members: Vec::new(),
        id: format!("marketplace:claude:{}", m.name),
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Claude],
        scope: Scope::Global,
        group: m.name.clone(),
        source: m.source.clone(),
        installer: Installer::ClaudeCli,
        origin: Default::default(),
        children: plugin_names(&own),
        removal: marketplace_removal(claude, m, &own),
    }
}

fn plugin_names(installs: &[&Install]) -> Vec<String> {
    let names: BTreeSet<String> = installs
        .iter()
        .map(|i| split_plugin_id(&i.id).0.to_string())
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

/// The Claude marketplace `src` carries, if any.
pub(super) fn plugin_offer(src: &Path) -> anyhow::Result<Option<PluginOffer>> {
    read_marketplace(&src.join(".claude-plugin/marketplace.json"))
}

/// Add `source` as a marketplace and install the `picked` plugins of `offer`.
pub(super) fn plugin_steps(
    offer: &PluginOffer,
    picked: &[String],
    source: &str,
    scope: Scope,
    cwd: &Path,
) -> Vec<Action> {
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "project",
    };
    install_commands(source, &offer.marketplace, picked, scope)
        .into_iter()
        .map(|(program, args)| Action::required(&program, args, cwd))
        .collect()
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

/// A profile's plugin entry for `scope`; `None` for other entries. A project's plugins go in
/// the `local` scope, so they never touch the settings file the team shares.
pub(super) fn provision(entry: &Spec, scope: Scope, cwd: &Path) -> Option<Vec<Action>> {
    let Spec::Plugin {
        source,
        marketplace,
        name,
        ..
    } = entry
    else {
        return None;
    };
    let scope = match scope {
        Scope::Global => "user",
        Scope::Project => "local",
    };
    let commands = install_commands(source, marketplace, std::slice::from_ref(name), scope);
    Some(
        commands
            .into_iter()
            .map(|(program, args)| Action::required(&program, args, cwd))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::Claude;
    use super::super::files::installed_file;
    use super::super::testutil::{by_id, dirs, files, fixture, remove_install, remove_key, rm};
    use super::*;
    use crate::agents::AgentAdapter;
    use crate::install::test_support::{marketplace_fixture, project_with, s};
    use std::path::PathBuf;

    fn mp() -> PluginOffer {
        PluginOffer {
            marketplace: "mp".into(),
            plugins: s(&["p1", "p2"]),
        }
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
            version: None,
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
    fn offer_is_the_source_marketplace() {
        let fx = project_with(marketplace_fixture);

        let found = plugin_offer(&fx.env.project.join("repo")).unwrap();

        assert_eq!(found, Some(mp()));
        assert_eq!(plugin_offer(&fx.env.project).unwrap(), None);
    }

    #[test]
    fn steps_add_the_marketplace_then_install_each_pick() {
        let cwd = Path::new("/p");

        let steps = plugin_steps(&mp(), &s(&["p1", "p2"]), "o/r", Scope::Project, cwd);

        assert_eq!(
            steps,
            vec![
                Action::required("claude", s(&["plugin", "marketplace", "add", "o/r"]), cwd),
                Action::required(
                    "claude",
                    s(&["plugin", "install", "p1@mp", "--scope", "project"]),
                    cwd
                ),
                Action::required(
                    "claude",
                    s(&["plugin", "install", "p2@mp", "--scope", "project"]),
                    cwd
                ),
            ]
        );
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
}
