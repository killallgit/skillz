//! What the list shows: the project and the user's installs read from their native config, and
//! the library.

use std::path::Path;

use super::super::session::{Notice, Session};
use crate::app::library::Catalog;
use crate::app::workspace::Model;
use crate::cli::scope::{installed, project_env};
use skillz_harness::model::Scope;

/// The snapshot to show next, with `notice` to toast; an unreadable library is shown empty and
/// reported in its place.
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
        agents: deps
            .adapters
            .iter()
            .map(|adapter| adapter.agent())
            .collect(),
        library,
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
    fn local_sources_match_in_the_list_without_rewriting_stored_entries() {
        use crate::app::library::{Catalog, Held, LibraryItem};
        use crate::app::store::Store;
        use crate::cli::deps::Deps;
        use skillz_harness::Spec;
        use skillz_harness::env::Env;
        use skillz_harness::model::Agent;
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
        let default = LibraryItem::from_entry(entry("reviewer"));
        let custom = LibraryItem::new("My helper", entry("helper"));
        let store = Store::in_env(&env);
        store
            .update(|document| {
                document.items = Catalog::from_items(vec![default.clone(), custom.clone()]);
                Ok(())
            })
            .unwrap();
        let before = fs::read(crate::app::app_paths::store_path(&env)).unwrap();
        let prompter = crate::app::prompt::AssumeYes;
        let mut deps = Deps::for_test(
            dir.path(),
            skillz_harness::agents::registry(),
            (&skillz_harness::runner::NoCommands, &prompter),
        );
        deps.cwd = project;

        let model = snapshot(&Session::open(&deps, &env), None).unwrap();

        assert!(
            Held::of(&model.installed, &model.global)
                .presence(&model.library.items()[0].entry)
                .project
        );
        let resolved = |entry| crate::app::stale::with_source(entry, canonical.to_str().unwrap());
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
            fs::read(crate::app::app_paths::store_path(&env)).unwrap(),
            before
        );
    }
}
