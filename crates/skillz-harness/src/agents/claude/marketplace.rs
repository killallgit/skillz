//! Claude marketplaces: discovering the ones installed, and installing a source's plugins.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::files::{EXTRA_KNOWN_MARKETPLACES, marketplaces_file, read_json, settings_files};
use super::records::{Install, Marketplace, marketplaces_from, same_path};
use super::removal::{plugin_removal, unique_in_order};
use crate::Spec;
use crate::action::{Action, Edit, RemovalTarget};
use crate::agents::PluginOffer;
use crate::agents::plugin::{declared_skill_paths, read_marketplace, tolerate_malformed};
use crate::env::Env;
use crate::model::{Agent, Installed, Installer, Kind, Scope, split_plugin_id};
use crate::runner::Cmd;

/// Marketplace declarations belong to settings scopes; the registry is shared download metadata.
pub(super) fn installed(
    env: &Env,
    scope: Scope,
    markets: &[Marketplace],
    installs: &[Install],
) -> anyhow::Result<Vec<Installed>> {
    let mut declarations = Vec::new();
    for declared_scope in [Scope::Global, Scope::Project] {
        for file in settings_files(env, declared_scope) {
            // A malformed settings file declares nothing here; hook discovery reports it, and
            // one broken project file must not hide the user's own plugins.
            let Ok(settings) = read_json(&file) else {
                continue;
            };
            for market in marketplaces_from(&settings[EXTRA_KNOWN_MARKETPLACES]) {
                declarations.push((declared_scope, file.clone(), market));
            }
        }
    }
    let mut names = BTreeSet::new();
    names.extend(declarations.iter().map(|(_, _, m)| m.name.clone()));
    let mut out = Vec::new();
    for name in names {
        let declared: Vec<_> = declarations
            .iter()
            .filter(|(_, _, m)| m.name == name)
            .collect();
        let own: Vec<_> = installs
            .iter()
            .filter(|i| split_plugin_id(&i.id).1 == name)
            .collect();
        let Some((_, _, definition)) = declared.iter().find(|(s, _, _)| *s == scope) else {
            continue;
        };
        // Where the clone lives is Claude's record, never a repo's settings file: removal
        // deletes that directory.
        let mut market = definition.clone();
        market.install_location = markets
            .iter()
            .find(|m| m.name == name)
            .map(|cached| cached.install_location.clone())
            .unwrap_or_default();
        let shared = declared.iter().any(|(s, _, _)| *s != scope)
            || own.iter().any(|i| !in_scope(env, scope, i));
        let own: Vec<Install> = own
            .into_iter()
            .filter(|i| in_scope(env, scope, i))
            .cloned()
            .collect();
        let files: Vec<PathBuf> = declared
            .iter()
            .filter(|(s, _, _)| *s == scope)
            .map(|(_, file, _)| file.clone())
            .collect();
        let removal = scoped_removal(env, &market, (&files, scope), &own, shared);
        out.push(installed_item(scope, &market, &own, removal));
    }
    Ok(out)
}

fn in_scope(env: &Env, scope: Scope, install: &Install) -> bool {
    match (scope, &install.project) {
        (Scope::Global, None) => true,
        (Scope::Project, Some(project)) => same_path(project, &env.project),
        _ => false,
    }
}

/// `declared` are the settings files of `scope` that declare `market`: the CLI fails on a
/// scope that does not, so only these get a removal step.
fn scoped_removal(
    env: &Env,
    market: &Marketplace,
    (declared, scope): (&[PathBuf], Scope),
    installs: &[Install],
    shared: bool,
) -> Vec<Action> {
    let claude = super::dir(env);
    let mut steps = unique_in_order(installs.iter().flat_map(|i| plugin_removal(&claude, i)));
    for file in declared.iter().cloned() {
        // A shared registry and clone can still serve other projects. Only the selected
        // declarations and plugin records are ours to remove in that case.
        if !shared {
            let native_scope = match scope {
                Scope::Global => "user",
                Scope::Project if file.ends_with("settings.local.json") => "local",
                Scope::Project => "project",
            };
            steps.extend(Action::native(
                "claude",
                &[
                    "plugin",
                    "marketplace",
                    "remove",
                    &market.name,
                    "--scope",
                    native_scope,
                ],
                &[&market.name],
                &env.project,
            ));
        }
        steps.push(Action::Edit {
            file,
            edit: Edit::JsonRemoveKey {
                path: vec![EXTRA_KNOWN_MARKETPLACES.into(), market.name.clone()],
            },
        });
    }
    if !shared {
        steps.push(Action::Edit {
            file: marketplaces_file(&claude),
            edit: Edit::JsonRemoveKey {
                path: vec![market.name.clone()],
            },
        });
        steps.extend(owned_clone(&claude, market).map(Action::RemovePath));
    }
    steps
}

/// One declaration with its scope and the plugins installed there.
fn installed_item(
    scope: Scope,
    m: &Marketplace,
    installs: &[Install],
    removal: Vec<Action>,
) -> Installed {
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
        id: match scope {
            Scope::Global => format!("marketplace:claude:{}", m.name),
            Scope::Project => format!("marketplace:claude:project:{}", m.name),
        },
        name: m.name.clone(),
        kind: Kind::Marketplace,
        agents: vec![Agent::Claude],
        scope,
        group: m.name.clone(),
        source: m.source.clone(),
        installer: Installer::ClaudeCli,
        origin: Default::default(),
        children: plugin_names(&own),
        removal,
    }
}

fn plugin_names(installs: &[&Install]) -> Vec<String> {
    let names: BTreeSet<String> = installs
        .iter()
        .map(|i| split_plugin_id(&i.id).0.to_string())
        .collect();
    names.into_iter().collect()
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
    tolerate_malformed(read_marketplace(
        &src.join(".claude-plugin/marketplace.json"),
        src,
        &[".claude-plugin/plugin.json"],
        plugin_skill_paths,
    ))
}

fn plugin_skill_paths(
    entry: &serde_json::Value,
    manifest: Option<&serde_json::Value>,
    from_marketplace_root: bool,
) -> Vec<String> {
    if manifest.is_some()
        && entry["strict"] == false
        && [
            "commands",
            "agents",
            "skills",
            "hooks",
            "outputStyles",
            "themes",
        ]
        .iter()
        .any(|key| entry.get(key).is_some())
    {
        return vec![];
    }
    let entry_paths = declared_skill_paths(entry);
    // Root-sourced marketplace entries can expose separate subsets of a shared skills tree.
    let mut paths = if from_marketplace_root && entry_paths.is_some() {
        vec![]
    } else {
        vec!["./skills".into()]
    };
    if let Some(manifest_paths) = manifest.and_then(declared_skill_paths) {
        paths.extend(manifest_paths);
    }
    paths.extend(entry_paths.into_iter().flatten());
    paths
}

/// The `--scope` a plugin installs at: project installs go in the private `local` scope, so
/// only this project has them and nothing is committed.
fn install_scope(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "user",
        Scope::Project => "local",
    }
}

/// Add `source` as a marketplace and install the `picked` plugins of `offer`.
pub(super) fn plugin_steps(
    offer: &PluginOffer,
    picked: &[String],
    source: &str,
    scope: Scope,
    cwd: &Path,
) -> Vec<Action> {
    let scope = install_scope(scope);
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
    std::iter::once(claude(&[
        "plugin",
        "marketplace",
        "add",
        source,
        "--scope",
        scope,
    ]))
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
    let scope = install_scope(scope);
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
    use super::super::testutil::{by_id, fixture, write};
    use super::*;
    use crate::action::execute;
    use crate::agents::Harness;
    use crate::runner::NoCommands;
    use crate::test_support::{marketplace_fixture, project_with, s};
    use serde_json::json;

    #[test]
    fn a_marketplace_file_that_does_not_parse_offers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir.path().join(".claude-plugin/marketplace.json"),
            "{ not json",
        );

        assert!(plugin_offer(dir.path()).unwrap().is_none());
    }

    fn mp() -> PluginOffer {
        PluginOffer {
            marketplace: "mp".into(),
            plugins: s(&["p1", "p2"]),
            skills: Default::default(),
            refused: vec![],
        }
    }

    fn declare(file: &Path, name: &str) {
        write(
            file,
            &json!({"extraKnownMarketplaces": {name: {
                "source": {"source": "github", "repo": "o/market"}
            }}})
            .to_string(),
        );
    }

    #[test]
    fn project_declarations_do_not_become_global_installs() {
        let f = fixture();
        write(&super::super::dir(&f.env).join("settings.json"), "{}");
        let local = f.env.project.join(".claude/settings.local.json");
        declare(&local, "acme");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let project = Claude.installed(&f.env, Scope::Project).unwrap();
        assert!(
            !global
                .iter()
                .any(|i| i.kind == Kind::Marketplace && i.name == "acme")
        );
        let market = by_id(&project, "marketplace:claude:project:acme");
        assert_eq!(market.scope, Scope::Project);
        assert_eq!(market.children, ["tools"]);
        assert_eq!(market.source.as_deref(), Some("o/market"));
    }

    #[test]
    fn shared_cache_metadata_is_not_a_marketplace_declaration() {
        let f = fixture();
        write(&super::super::dir(&f.env).join("settings.json"), "{}");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        assert!(!global.iter().any(|i| i.kind == Kind::Marketplace));
    }

    #[test]
    fn a_marketplace_lists_only_plugins_installed_in_its_scope() {
        let f = fixture();
        declare(&super::super::dir(&f.env).join("settings.json"), "acme");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let market = by_id(&global, "marketplace:claude:acme");
        assert_eq!(market.children, ["tools"]);
    }

    #[test]
    fn removing_a_global_marketplace_preserves_project_installs_and_shared_metadata() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        declare(&claude.join("settings.json"), "acme");
        declare(&f.env.project.join(".claude/settings.local.json"), "acme");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let market = by_id(&global, "marketplace:claude:acme");
        execute(&market.removal, &NoCommands).unwrap();
        let installs =
            super::super::records::installs_from(&read_json(&installed_file(&claude)).unwrap());
        assert!(
            installs
                .iter()
                .any(|i| i.id == "tools@acme" && i.project.as_ref() == Some(&f.env.project))
        );
        assert!(
            !installs
                .iter()
                .any(|i| i.id == "tools@acme" && i.project.is_none())
        );
        assert!(f.cache.exists());
        assert!(
            read_json(&marketplaces_file(&claude))
                .unwrap()
                .get("acme")
                .is_some()
        );
        assert!(
            !Claude
                .installed(&f.env, Scope::Global)
                .unwrap()
                .iter()
                .any(|i| i.kind == Kind::Marketplace && i.name == "acme")
        );
        assert!(
            Claude
                .installed(&f.env, Scope::Project)
                .unwrap()
                .iter()
                .any(|i| i.kind == Kind::Marketplace && i.name == "acme")
        );
    }

    #[test]
    fn removing_a_project_marketplace_preserves_other_projects_and_the_user_install() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        declare(&f.env.project.join(".claude/settings.local.json"), "acme");
        let project = Claude.installed(&f.env, Scope::Project).unwrap();
        let market = by_id(&project, "marketplace:claude:project:acme");
        execute(&market.removal, &NoCommands).unwrap();
        let installs =
            super::super::records::installs_from(&read_json(&installed_file(&claude)).unwrap());
        assert!(
            installs
                .iter()
                .any(|i| i.id == "tools@acme" && i.project.is_none())
        );
        assert!(installs.iter().any(|i| i.id == "solo@acme"));
        assert!(
            !installs
                .iter()
                .any(|i| i.project.as_ref() == Some(&f.env.project))
        );
        assert!(f.cache.exists());
        assert!(
            !Claude
                .installed(&f.env, Scope::Project)
                .unwrap()
                .iter()
                .any(|i| i.kind == Kind::Marketplace && i.name == "acme")
        );
    }

    #[test]
    fn a_malformed_settings_file_does_not_hide_the_other_scope() {
        let f = fixture();
        declare(&super::super::dir(&f.env).join("settings.json"), "acme");
        write(&f.env.project.join(".claude/settings.local.json"), "{ nope");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        assert!(global.iter().any(|i| i.id == "marketplace:claude:acme"));
    }

    #[test]
    fn a_settings_file_cannot_pick_the_clone_that_removal_deletes() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        write(&installed_file(&claude), "{}");
        write(&marketplaces_file(&claude), "{}");
        let victim = claude.join("plugins/marketplaces/other");
        write(
            &claude.join("settings.json"),
            &json!({"extraKnownMarketplaces": {"evil": {
                "source": {"source": "github", "repo": "o/evil"},
                "installLocation": victim.to_string_lossy(),
            }}})
            .to_string(),
        );
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let market = by_id(&global, "marketplace:claude:evil");
        assert!(
            !market
                .removal
                .iter()
                .any(|action| matches!(action, Action::RemovePath(_)))
        );
    }

    #[test]
    fn a_shared_marketplace_plans_no_native_removal() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        declare(&claude.join("settings.json"), "acme");
        declare(&f.env.project.join(".claude/settings.local.json"), "acme");
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let market = by_id(&global, "marketplace:claude:acme");
        assert!(
            !market
                .removal
                .iter()
                .any(|action| matches!(action, Action::Run { args, .. } if args.contains(&"marketplace".to_string())))
        );
    }

    #[test]
    fn a_scope_is_only_asked_to_remove_a_marketplace_it_declares() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        write(&installed_file(&claude), "{}");
        write(&claude.join("settings.json"), "{}");
        declare(&f.env.project.join(".claude/settings.local.json"), "acme");
        let project = Claude.installed(&f.env, Scope::Project).unwrap();
        let market = by_id(&project, "marketplace:claude:project:acme");
        let scopes: Vec<_> = market
            .removal
            .iter()
            .filter_map(|action| match action {
                Action::Run { args, .. }
                    if args.starts_with(&s(&["plugin", "marketplace", "remove"])) =>
                {
                    args.last().cloned()
                }
                _ => None,
            })
            .collect();
        assert_eq!(scopes, ["local"]);
    }

    #[test]
    fn removing_an_unused_marketplace_drops_its_registry_and_owned_clone() {
        let f = fixture();
        let claude = super::super::dir(&f.env);
        write(&installed_file(&claude), "{}");
        declare(&claude.join("settings.json"), "acme");
        let clone = claude.join("plugins/marketplaces/acme");
        std::fs::create_dir_all(&clone).unwrap();
        let global = Claude.installed(&f.env, Scope::Global).unwrap();
        let market = by_id(&global, "marketplace:claude:acme");
        execute(&market.removal, &NoCommands).unwrap();
        assert!(!clone.exists());
        assert!(
            read_json(&marketplaces_file(&claude))
                .unwrap()
                .get("acme")
                .is_none()
        );
    }

    #[test]
    fn marketplace_keeps_locations_outside_its_owned_clone_root() {
        for location in [
            "/src/repo",
            "/h/.claude/plugins/marketplaces",
            "/h/.claude/plugins/marketplaces/../../..",
            "/h/.claude/plugins/marketplaces/a/b",
            "",
        ] {
            let market = Marketplace {
                name: "mp".into(),
                source: None,
                install_location: location.into(),
            };
            assert!(owned_clone(Path::new("/h/.claude"), &market).is_none());
        }
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
                Action::required(
                    "claude",
                    s(&["plugin", "marketplace", "add", "o/r", "--scope", "local"]),
                    cwd
                ),
                Action::required(
                    "claude",
                    s(&["plugin", "install", "p1@mp", "--scope", "local"]),
                    cwd
                ),
                Action::required(
                    "claude",
                    s(&["plugin", "install", "p2@mp", "--scope", "local"]),
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
