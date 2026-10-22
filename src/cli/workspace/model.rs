//! What the tabs show: the project read from its native config, the library and the profiles.

use std::path::Path;

use super::super::session::{Notice, Session};
use crate::cli::scope::{installed, project_env};
use crate::library::Catalog;
use crate::model::Scope;
use crate::tui::workspace::Model;

/// The snapshot to show next, with `notice` to toast; an unreadable library is shown empty and
/// reported in its place, along with the profiles kept beside it.
pub(super) fn snapshot(session: &Session, notice: Option<Notice>) -> anyhow::Result<Model> {
    let deps = session.deps();
    let env = project_env(deps, session.project());
    let (library, problem) = match session.library().read() {
        Ok(document) => (document.items, None),
        Err(e) => (Catalog::default(), Some(Notice::Failed(format!("{e:#}")))),
    };
    Ok(Model {
        dir: short_path(session.project(), &deps.home),
        installed: installed(deps, &env, Scope::Project),
        global: installed(deps, &env, Scope::Global),
        library,
        profiles: session.profiles().readable().unwrap_or_default(),
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
}
