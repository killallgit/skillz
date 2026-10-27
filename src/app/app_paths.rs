use std::path::{Path, PathBuf};

use skillz_harness::Spec;
use skillz_harness::env::Env;

/// skillz's own config: the store and the dirs `run` keeps.
pub fn skillz_dir(env: &Env) -> PathBuf {
    env.home.join(".config/skillz")
}

/// The one global store of the library and the profiles.
pub fn store_path(env: &Env) -> PathBuf {
    skillz_dir(env).join("store.json")
}

/// `text` with the home directory written `~`, wherever a path in it starts there. A path that
/// merely contains the home directory's name further along (`/srv/home/me/x`) is left alone.
pub fn tilde(text: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    if home.is_empty() || home == "/" {
        return text.to_string();
    }
    let inside = format!("{home}/");
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(&inside) {
        let starts_a_path = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | '~')));
        out.push_str(&rest[..at]);
        out.push_str(if starts_a_path { "~/" } else { &inside });
        rest = &rest[at + inside.len()..];
    }
    out.push_str(rest);
    match out == *home {
        true => "~".into(),
        false => out,
    }
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

/// A temporary spec with local sources resolved for the current project.
pub fn spec_in_env(env: &Env, mut spec: Spec) -> Spec {
    match &mut spec {
        Spec::Skill { source, .. } | Spec::Subagent { source, .. } => {
            *source = resolve_source(env, source)
        }
        _ => {}
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_directory_is_written_as_tilde_wherever_a_path_starts_there() {
        let home = Path::new("/home/me");

        let inside = tilde("skills tdd [/home/me/code/skills]", home);
        let exact = tilde("/home/me", home);
        let other = tilde("/srv/home/mex/app", home);
        let deeper = tilde("/srv/home/me/app", home);

        assert_eq!(inside, "skills tdd [~/code/skills]");
        assert_eq!(exact, "~");
        assert_eq!(other, "/srv/home/mex/app");
        assert_eq!(deeper, "/srv/home/me/app");
    }
}
