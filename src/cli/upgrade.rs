//! `skillz upgrade`: update the plugins of both agents, in the project and at user level, that
//! have something newer. What the check cannot tell is updated anyway: the tools' own update
//! commands do nothing when there is nothing new.

use std::io::Write;

use super::scope::Scene;
use super::session::{Change, change, with_session};
use skillz_harness::model::{Installed, Kind, Scope};
use skillz_harness::update;
use skillz_harness::update::check::UpdateCheck;

pub(super) struct UpgradeRequest {
    pub dry_run: bool,
}

pub(super) fn upgrade(
    scene: &Scene,
    request: &UpgradeRequest,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.complete()?;
    let due = due(scene.deps.checker.as_ref(), &items, &|item| {
        !update::steps(&scene.deps.adapters, item, &scene.env.project).is_empty()
    });
    if due.is_empty() {
        writeln!(out, "everything is up to date")?;
        return Ok(());
    }
    with_session(
        scene.deps,
        scene.env,
        out,
        request.dry_run,
        |session, console| {
            // A failure in one level does not keep the other from being upgraded.
            let mut failed = None;
            for &scope in scene.scopes {
                let ids = ids_in(&due, scope);
                if ids.is_empty() {
                    continue;
                }
                let notice = change(session, scope, &Change::Update(&ids))?;
                if let Err(e) = console.conclude(notice) {
                    failed.get_or_insert(e);
                }
            }
            failed.map_or(Ok(()), Err)
        },
    )
}

/// The plugins and marketplaces that are worth updating: they have an update step, and the
/// check does not say they are current. A plugin is as current as its marketplace.
fn due<'a>(
    checker: &dyn UpdateCheck,
    items: &'a [Installed],
    updatable: &dyn Fn(&Installed) -> bool,
) -> Vec<&'a Installed> {
    let parts: Vec<&Installed> = items.iter().flat_map(|item| item.parts()).collect();
    let stale = |part: &Installed| {
        let anchor = marketplace_of(part, &parts);
        !checker.can_tell(anchor) || checker.available(anchor)
    };
    items
        .iter()
        .filter(|item| matches!(item.kind, Kind::Plugin | Kind::Marketplace))
        .filter(|item| updatable(item))
        .filter(|item| item.parts().iter().any(stale))
        .collect()
}

/// The marketplace a plugin comes from, when it is installed too, else the item itself.
fn marketplace_of<'a>(item: &'a Installed, all: &[&'a Installed]) -> &'a Installed {
    if item.kind != Kind::Plugin {
        return item;
    }
    all.iter()
        .copied()
        .find(|other| {
            other.kind == Kind::Marketplace
                && other.group == item.group
                && other.installer == item.installer
        })
        .unwrap_or(item)
}

fn ids_in(due: &[&Installed], scope: Scope) -> Vec<String> {
    due.iter()
        .filter(|item| item.scope == scope)
        .flat_map(|item| item.parts().iter().map(|part| part.id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::install::test_support::{FakeRunner, ScriptedPrompter};
    use crate::cli::args::Filter;
    use crate::cli::deps::Deps;
    use skillz_harness::action::Action;
    use skillz_harness::agents::Harness;
    use skillz_harness::env::Env;
    use skillz_harness::model::{Agent, Installer};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    fn item(name: &str, kind: Kind, group: &str, scope: Scope, installer: Installer) -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
            id: format!("{scope}:{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope,
            group: group.into(),
            source: None,
            installer,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn claude(name: &str, kind: Kind, group: &str, scope: Scope) -> Installed {
        item(name, kind, group, scope, Installer::ClaudeCli)
    }

    /// Two Claude marketplaces, `mp` moved on and `fresh` not, with plugins of each at both
    /// levels, and a Codex plugin the check knows nothing about.
    fn installs() -> Vec<Installed> {
        vec![
            claude("mp", Kind::Marketplace, "mp", Scope::Global),
            claude("a@mp", Kind::Plugin, "mp", Scope::Global),
            claude("fresh", Kind::Marketplace, "fresh", Scope::Global),
            claude("b@fresh", Kind::Plugin, "fresh", Scope::Global),
            item(
                "c@cx",
                Kind::Plugin,
                "cx",
                Scope::Global,
                Installer::CodexCli,
            ),
            claude("d@mp", Kind::Plugin, "mp", Scope::Project),
            claude("e@fresh", Kind::Plugin, "fresh", Scope::Project),
        ]
    }

    /// Knows the state of Claude marketplaces only; `mp` has an update.
    struct Moved;

    impl UpdateCheck for Moved {
        fn available(&self, item: &Installed) -> bool {
            item.name == "mp"
        }

        fn can_tell(&self, item: &Installed) -> bool {
            item.kind == Kind::Marketplace && item.installer == Installer::ClaudeCli
        }
    }

    fn names(items: &[&Installed]) -> Vec<String> {
        items.iter().map(|i| i.name.clone()).collect()
    }

    #[test]
    fn a_plugin_is_due_with_its_marketplace_and_what_cannot_be_checked_always_is() {
        let items = installs();

        let due = due(&Moved, &items, &|_| true);

        assert_eq!(names(&due), ["mp", "a@mp", "c@cx", "d@mp"]);
    }

    #[test]
    fn what_has_no_update_step_is_not_due() {
        let items = installs();

        let due = due(&Moved, &items, &|i| i.name != "c@cx");

        assert_eq!(names(&due), ["mp", "a@mp", "d@mp"]);
    }

    #[test]
    fn only_plugins_and_marketplaces_are_due() {
        let items = [claude("tdd", Kind::Skill, "g", Scope::Global)];

        assert!(due(&skillz_harness::update::check::NoCheck, &items, &|_| true).is_empty());
    }

    #[test]
    fn with_the_check_switched_off_everything_updatable_is_due() {
        let items = installs();

        let due = due(&skillz_harness::update::check::NoCheck, &items, &|_| true);

        assert_eq!(due.len(), items.len());
    }

    #[test]
    fn ids_are_split_by_level() {
        let items = installs();
        let due = due(&Moved, &items, &|_| true);

        let project = ids_in(&due, Scope::Project);
        let global = ids_in(&due, Scope::Global);

        assert_eq!(project, [items[5].id.clone()]);
        assert_eq!(global.len(), 3);
    }

    struct Fake;

    impl Harness for Fake {
        fn agent(&self) -> Agent {
            Agent::Claude
        }
        fn hook_files(&self, _: &Env, _: Scope) -> Vec<PathBuf> {
            vec![]
        }
        fn installed(&self, _: &Env, scope: Scope) -> anyhow::Result<Vec<Installed>> {
            Ok(installs()
                .into_iter()
                .filter(|item| item.scope == scope)
                .collect())
        }
        fn home_dir_names(&self) -> &'static [&'static str] {
            &[]
        }
        fn plan_update(&self, item: &Installed, cwd: &Path) -> Vec<Action> {
            vec![Action::required(
                "fake",
                vec!["update".into(), item.name.clone()],
                cwd,
            )]
        }
    }

    fn upgraded(approve: bool, dry_run: bool) -> Vec<String> {
        let (home, project) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let runner = FakeRunner::new(|_| {});
        let prompter = ScriptedPrompter::new(&[], approve);
        let mut deps = Deps::for_test(home.path(), vec![Box::new(Fake)], (&runner, &prompter));
        deps.cwd = project.path().to_path_buf();
        deps.checker = Arc::new(Moved);
        let env = super::super::scope::resolve_project(&deps, None).unwrap();
        let scopes = [Scope::Global, Scope::Project];
        let filter = Filter::default();
        let scene = Scene::new(&deps, &env, &scopes, &filter);
        let mut out = Vec::new();

        upgrade(&scene, &UpgradeRequest { dry_run }, &mut out).unwrap();

        runner
            .calls()
            .iter()
            .map(|(_, args, _)| args[1].clone())
            .collect()
    }

    #[test]
    fn upgrading_updates_each_level_and_leaves_what_is_current() {
        let ran = upgraded(true, false);

        let mut sorted = ran.clone();
        sorted.sort();
        assert_eq!(sorted, ["a@mp", "c@cx", "d@mp", "mp"]);
    }

    #[test]
    fn a_declined_or_dry_upgrade_runs_nothing() {
        let declined = upgraded(false, false);
        let dry = upgraded(true, true);

        assert!(declined.is_empty() && dry.is_empty());
    }
}
