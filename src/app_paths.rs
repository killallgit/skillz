use std::path::{Path, PathBuf};

use crate::env::Env;

/// skillz's own config: the store and the dirs `run` keeps.
pub fn skillz_dir(env: &Env) -> PathBuf {
    env.home.join(".config/skillz")
}

/// The one global store of the library and the profiles.
pub fn store_path(env: &Env) -> PathBuf {
    skillz_dir(env).join("store.json")
}

/// Resolve local skill and subagent sources for this project. Repository references stay
/// portable; explicit local paths stay local even after their directory disappears.
pub fn resolve_source(env: &Env, source: &str) -> String {
    if source.contains("://") || source.starts_with("git@") {
        return source.to_string();
    }
    let path = env.project.join(source);
    if path.is_dir() {
        return std::fs::canonicalize(&path)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
    }
    if Path::new(source).is_absolute()
        || matches!(source, "." | "..")
        || source.starts_with("./")
        || source.starts_with("../")
    {
        return std::path::absolute(&path)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
    }
    source.to_string()
}
