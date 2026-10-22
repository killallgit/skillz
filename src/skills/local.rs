use std::path::Path;

use super::entry::canonical_or;
use super::frontmatter::skill_name;
use crate::env::Env;
use crate::model::Origin;

/// What a hand-made skill can be installed again from: the directory holding it, and the
/// name npx finds it by when that is not its directory name.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct LocalSource {
    pub origin: Origin,
    pub install_name: Option<String>,
}

/// Nothing when the skill lies outside the home and the project, or npx would not see it.
pub(super) fn local_source(env: &Env, canonical: &Path) -> LocalSource {
    let Some(parent) = canonical.parent() else {
        return LocalSource::default();
    };
    let Some(name) = skill_name(canonical).filter(|_| inside_home_or_project(env, canonical))
    else {
        return LocalSource::default();
    };
    LocalSource {
        origin: Origin {
            link: Some(parent.to_string_lossy().into_owned()),
            version: None,
        },
        install_name: Some(name),
    }
}

fn inside_home_or_project(env: &Env, path: &Path) -> bool {
    let roots = [&env.home, &env.project, &env.claude_dir(), &env.codex_dir()];
    roots
        .into_iter()
        .any(|root| path.starts_with(canonical_or(root)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::testutil::*;
    use super::*;

    fn named_skill(dir: &Path, md: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), md).unwrap();
    }

    #[test]
    fn a_skill_in_the_home_is_installed_again_from_the_dir_holding_it() {
        let f = fixture();
        let dir = f.env.home.join(".claude/skills/mine");
        named_skill(&dir, "---\nname: mine\ndescription: d\n---\n");

        let found = local_source(&f.env, &dir);

        let parent = f.env.home.join(".claude/skills");
        assert_eq!(found.origin.link, Some(parent.to_string_lossy().into()));
        assert_eq!(found.install_name.as_deref(), Some("mine"));
    }

    #[test]
    fn discovery_gives_a_hand_made_skill_its_dir_but_a_locked_one_keeps_its_own_origin() {
        let f = fixture();
        let mine = f.env.home.join(".claude/skills/mine");
        named_skill(&mine, "---\nname: mine\ndescription: d\n---\n");
        let locked = f.env.home.join(".agents/skills/tdd");
        named_skill(&locked, "---\nname: tdd\ndescription: d\n---\n");
        write_lock(
            &f.env.home.join(".agents/.skill-lock.json"),
            r#"{"skills":{"tdd":{"source":"o/r","sourceUrl":"https://h/o/r.git"}}}"#,
        );

        let augs = crate::skills::discover(&f.env, &global_dirs(&f.env)).unwrap();

        let link = |name: &str| by_name(&augs, name).origin.link.clone();
        let held_in = f.env.home.join(".claude/skills");
        assert_eq!(link("mine"), Some(held_in.to_string_lossy().into()));
        assert_eq!(link("tdd"), Some("https://h/o/r.git".into()));
    }

    #[test]
    fn a_skill_npx_names_differently_carries_that_name() {
        let f = fixture();
        let dir = f.env.project.join(".agents/skills/folder");
        named_skill(&dir, "---\nname: real\ndescription: d\n---\n");

        let found = local_source(&f.env, &dir);

        assert_eq!(found.install_name.as_deref(), Some("real"));
    }

    #[test]
    fn a_skill_npx_would_skip_has_no_source() {
        let f = fixture();
        let dir = f.env.home.join(".claude/skills/bare");
        named_skill(&dir, "---\nname: bare\n---\n");

        assert_eq!(local_source(&f.env, &dir), LocalSource::default());
    }

    #[test]
    fn a_skill_outside_the_home_and_project_has_no_source() {
        let f = fixture();
        let elsewhere = tempfile::tempdir().unwrap();
        let dir = fs::canonicalize(elsewhere.path()).unwrap().join("mine");
        named_skill(&dir, "---\nname: mine\ndescription: d\n---\n");

        assert_eq!(local_source(&f.env, &dir), LocalSource::default());
    }
}
