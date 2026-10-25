//! What the tabs show: the project read from its native config, the library and the profiles.

use std::path::Path;

use super::super::session::{Notice, Session};
use crate::cli::scope::{installed, project_env};
use crate::harness::model::Scope;
use crate::library::Catalog;
use crate::workspace::Model;

/// The snapshot to show next, with `notice` to toast; an unreadable library is shown empty and
/// reported in its place, along with the profiles kept beside it.
pub(super) fn snapshot(session: &Session, notice: Option<Notice>) -> anyhow::Result<Model> {
    let deps = session.deps();
    let env = project_env(deps, session.project());
    let (library, problem) = match session.library().read() {
        Ok(document) => (document.items.in_env(&env), None),
        Err(e) => (Catalog::default(), Some(Notice::Failed(format!("{e:#}")))),
    };
    Ok(Model {
        dir: short_path(session.project(), &deps.home),
        installed: installed(deps, &env, Scope::Project),
        global: installed(deps, &env, Scope::Global),
        library,
        profiles: session
            .profiles()
            .readable()
            .unwrap_or_default()
            .into_iter()
            .map(|profile| profile.in_env(&env))
            .collect(),
        applied: session.applied(),
        notice: notice.or(problem).map(|n| n.to_string()),
    })
}

fn short_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_under_home_are_abbreviated_and_others_left_alone() {
        let home = Path::new("/home/me");

        let inside = short_path(Path::new("/home/me/app"), home);
        let outside = short_path(Path::new("/srv/app"), home);

        assert_eq!((inside.as_str(), outside.as_str()), ("~/app", "/srv/app"));
    }
    #[test]
    fn local_sources_match_in_workspace_views_without_rewriting_stored_entries() {
        use crate::cli::deps::Deps;
        use crate::harness::Spec;
        use crate::harness::env::Env;
        use crate::harness::model::Agent;
        use crate::library::{Catalog, Held, LibraryItem};
        use crate::profile::Profile;
        use crate::store::Store;
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let source = project.join("agents-src");
        let agents = project.join(".claude/agents");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&agents).unwrap();
        let canonical = fs::canonicalize(&source).unwrap();
        fs::write(
            agents.join("reviewer.md"),
            "---\nname: Reviewer\n---\nbody\n",
        )
        .unwrap();
        fs::write(
            agents.join(".skillz-sources.json"),
            serde_json::to_vec(&serde_json::json!({"reviewer.md": canonical})).unwrap(),
        )
        .unwrap();
        let env = Env::new(dir.path().into(), project.clone());
        let entry = |name: &str| Spec::Subagent {
            source: "./agents-src".into(),
            names: vec![name.into()],
            agents: vec![Agent::Claude],
        };
        let mut profile = Profile::new("local");
        profile.entries = vec![entry("reviewer")];
        let default = LibraryItem::from_entry(entry("reviewer"));
        let custom = LibraryItem::new("My helper", entry("helper"));
        let store = Store::in_env(&env);
        store
            .update(|document| {
                document.profiles = vec![profile];
                document.items = Catalog::from_items(vec![default.clone(), custom.clone()]);
                Ok(())
            })
            .unwrap();
        let before = fs::read(crate::app_paths::store_path(&env)).unwrap();
        let prompter = crate::prompt::AssumeYes;
        let mut deps = Deps::for_test(
            dir.path(),
            crate::harness::agents::registry(),
            (&crate::harness::runner::NoCommands, &prompter),
        );
        deps.cwd = project;

        let model = snapshot(&Session::open(&deps, &env), None).unwrap();

        assert!(crate::apply::drift(&model.profiles[0], &model.installed).matches());
        assert!(crate::apply::loaded(&model.profiles, &model.installed).is_some());
        assert!(
            Held::of(&model.installed, &model.global)
                .presence(&model.library.items()[0].entry)
                .project
        );
        let resolved = |entry| crate::stale::with_source(entry, canonical.to_str().unwrap());
        assert_eq!(
            model.library.items()[0],
            LibraryItem::from_entry(resolved(default.entry))
        );
        assert_eq!(
            model.library.items()[1],
            LibraryItem {
                entry: resolved(custom.entry.clone()),
                ..custom
            }
        );
        assert_eq!(
            fs::read(crate::app_paths::store_path(&env)).unwrap(),
            before
        );
    }

    #[test]
    fn explicit_missing_sources_stay_local_and_repository_references_stay_portable() {
        use crate::cli::deps::Deps;
        use crate::harness::Spec;
        use crate::harness::env::Env;
        use crate::profile::Profile;
        use crate::stale::{Disk, missing_source};
        use crate::store::Store;
        let dir = tempfile::tempdir().unwrap();
        let env = Env::new(dir.path().into(), dir.path().into());
        let mut profile = Profile::new("sources");
        profile.entries = [
            "./gone",
            "../gone",
            "org/repo",
            "https://host/repo.git",
            "git@host:org/repo.git",
        ]
        .into_iter()
        .map(|source| Spec::Skill {
            source: source.into(),
            names: vec!["reviewer".into()],
            agents: vec![],
        })
        .collect();
        let original = profile.clone();
        Store::in_env(&env)
            .update(|document| {
                document.profiles.push(profile);
                Ok(())
            })
            .unwrap();
        let prompter = crate::prompt::AssumeYes;
        let deps = Deps::for_test(
            dir.path(),
            vec![],
            (&crate::harness::runner::NoCommands, &prompter),
        );

        let model = snapshot(&Session::open(&deps, &env), None).unwrap();
        let entries = &model.profiles[0].entries;

        assert!(
            entries[..2]
                .iter()
                .all(|entry| missing_source(entry, &Disk).is_some())
        );
        assert_eq!(&entries[2..], &original.resolved()[2..]);
    }
}
