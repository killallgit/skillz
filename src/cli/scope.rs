use std::path::{Component, Path, PathBuf};

use anyhow::Context;

use super::args::Filter;
use super::deps::Deps;
use crate::agents;
use crate::env::Env;
use crate::merge::merge;
use crate::model::{Installed, Scope};

/// The environment of the project at `path` (default: cwd).
pub(super) fn resolve_project(deps: &Deps, path: Option<&Path>) -> anyhow::Result<Env> {
    let project = match path {
        Some(p) => deps.cwd.join(p),
        None => deps.cwd.clone(),
    };
    let project = project
        .canonicalize()
        .with_context(|| format!("no such directory: {}", project.display()))?;
    let home = deps
        .home
        .canonicalize()
        .unwrap_or_else(|_| deps.home.clone());
    Ok(Env {
        home,
        ..project_env(deps, &project)
    })
}

/// Scope rule for discovery: a path alone means that project, `-g` alone means global, neither
/// means both. In the home dir "project" files are the global ones, so that project is global.
pub(super) fn resolve(
    deps: &Deps,
    path: Option<&Path>,
    global: bool,
) -> anyhow::Result<(Env, Vec<Scope>)> {
    let env = resolve_project(deps, path)?;
    let scopes = match (env.project == env.home, path.is_some(), global) {
        (true, _, _) | (false, false, true) => vec![Scope::Global],
        (false, true, false) => vec![Scope::Project],
        _ => vec![Scope::Global, Scope::Project],
    };
    Ok((env, scopes))
}

/// What is installed and passes the filters, one item per thing however many agents have it.
pub(super) fn gather(deps: &Deps, env: &Env, scopes: &[Scope], filter: &Filter) -> Vec<Installed> {
    merge(gather_per_agent(deps, env, scopes, filter))
}

/// One item per agent copy, before logical items are merged.
pub(super) fn gather_per_agent(
    deps: &Deps,
    env: &Env,
    scopes: &[Scope],
    filter: &Filter,
) -> Vec<Installed> {
    let inv = agents::inventory(&deps.adapters, env, scopes);
    for warning in &inv.warnings {
        eprintln!("warning: {warning}");
    }
    inv.items.into_iter().filter(|i| filter.keeps(i)).collect()
}

/// `list TARGET`: a path starting with `.`, `..` or `/` is the project (as in
/// `list --hooks .`); anything else, even a word naming a directory, is a query. With `-p`,
/// TARGET is always a query.
pub(super) fn split_target(
    project: Option<PathBuf>,
    target: Option<String>,
) -> (Option<PathBuf>, Option<String>) {
    let path_like = |t: &str| {
        matches!(
            Path::new(t).components().next(),
            Some(Component::CurDir | Component::ParentDir | Component::RootDir)
        )
    };
    match (project, target) {
        (None, Some(t)) if path_like(&t) => (Some(PathBuf::from(t)), None),
        other => other,
    }
}

/// What the removal flows share: where to look and what the filters allow.
pub(super) struct Scene<'a, 'd> {
    pub(super) deps: &'a Deps<'d>,
    pub(super) env: &'a Env,
    pub(super) scopes: &'a [Scope],
    pub(super) filter: &'a Filter,
}

impl<'a, 'd> Scene<'a, 'd> {
    pub(super) fn new(
        deps: &'a Deps<'d>,
        env: &'a Env,
        scopes: &'a [Scope],
        filter: &'a Filter,
    ) -> Self {
        Scene {
            deps,
            env,
            scopes,
            filter,
        }
    }

    pub(super) fn gather(&self) -> Vec<Installed> {
        gather(self.deps, self.env, self.scopes, self.filter)
    }

    pub(super) fn complete(&self) -> anyhow::Result<Vec<Installed>> {
        let items = agents::inventory(&self.deps.adapters, self.env, self.scopes).complete()?;
        Ok(merge(
            items
                .into_iter()
                .filter(|item| self.filter.keeps(item))
                .collect(),
        ))
    }

    pub(super) fn gather_per_agent(&self) -> Vec<Installed> {
        gather_per_agent(self.deps, self.env, self.scopes, self.filter)
    }
}

/// Everything installed in one scope of `env`, whatever the filters.
pub(super) fn installed(deps: &Deps, env: &Env, scope: Scope) -> Vec<Installed> {
    gather(deps, env, &[scope], &Filter::default())
}

pub(super) fn complete_installed(
    deps: &Deps,
    env: &Env,
    scope: Scope,
) -> anyhow::Result<Vec<Installed>> {
    agents::inventory(&deps.adapters, env, &[scope])
        .complete()
        .map(merge)
}

/// The environment of one project, with the config dirs this run redirects.
pub(super) fn project_env(deps: &Deps, dir: &Path) -> Env {
    Env {
        project: dir.to_path_buf(),
        claude_config: deps.claude_config.clone(),
        codex_home: deps.codex_home.clone(),
        ..Env::new(deps.home.clone(), dir.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::AssumeYes;
    use crate::runner::SystemRunner;

    static NO_ENVS: SystemRunner = SystemRunner { envs: Vec::new() };

    fn deps_at(home: &Path, cwd: &Path) -> Deps<'static> {
        Deps {
            home: home.to_path_buf(),
            claude_config: None,
            codex_home: None,
            cwd: cwd.to_path_buf(),
            adapters: Vec::new().into(),
            runner: &NO_ENVS,
            prompter: &AssumeYes,
            interactive: false,
            checker: std::sync::Arc::new(crate::update::check::NoCheck),
            scanner: None,
        }
    }

    fn scopes(deps: &Deps, path: Option<&str>, global: bool) -> Vec<Scope> {
        resolve(deps, path.map(Path::new), global).unwrap().1
    }

    #[test]
    fn resolve_home_as_project_is_global() {
        let home = tempfile::tempdir().unwrap();
        let deps = deps_at(home.path(), home.path());
        assert_eq!(scopes(&deps, None, false), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("."), false), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("."), true), [Scope::Global]);
    }

    #[test]
    fn resolve_scope_rule_outside_home() {
        let (home, cwd) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        std::fs::create_dir(cwd.path().join("sub")).unwrap();
        let deps = deps_at(home.path(), cwd.path());
        let both = [Scope::Global, Scope::Project];
        assert_eq!(scopes(&deps, None, false), both);
        assert_eq!(scopes(&deps, None, true), [Scope::Global]);
        assert_eq!(scopes(&deps, Some("sub"), false), [Scope::Project]);
        assert_eq!(scopes(&deps, Some("sub"), true), both);
        let (env, _) = resolve(&deps, Some(Path::new("sub")), false).unwrap();
        assert_eq!(env.project, cwd.path().join("sub").canonicalize().unwrap());
        assert!(resolve(&deps, Some(Path::new("missing")), false).is_err());
    }

    fn split(project: Option<&str>, target: &str) -> (Option<PathBuf>, Option<String>) {
        split_target(project.map(PathBuf::from), Some(target.into()))
    }

    #[test]
    fn split_target_path_like_is_project() {
        for t in [".", "..", "./sub", "../sib", "/abs/dir"] {
            assert_eq!(split(None, t), (Some(PathBuf::from(t)), None), "{t}");
        }
    }

    #[test]
    fn split_target_word_is_query() {
        for t in ["caveman", "org/repo", ".hidden"] {
            assert_eq!(split(None, t), (None, Some(t.into())), "{t}");
        }
        assert_eq!(
            split(Some("p"), "."),
            (Some(PathBuf::from("p")), Some(".".into()))
        );
    }
}
