//! Entries whose source is a local path that no longer exists, because the folder was moved or
//! deleted. Git and URL sources are never stale: they are not paths.

use std::path::Path;

use skillz_harness::Spec;
use skillz_harness::agent_skills::skill_names_under;

/// What is on disk; injected so the rules can be tested without a disk.
pub trait Probe {
    fn exists(&self, path: &Path) -> bool;

    /// Whether the directory holds a skill called `name`. A probe that cannot tell says yes.
    fn holds_skill(&self, _dir: &Path, _name: &str) -> bool {
        true
    }
}

pub struct Disk;

impl Probe for Disk {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn holds_skill(&self, dir: &Path, name: &str) -> bool {
        skill_names_under(dir).map_or(true, |names| names.iter().any(|n| n == name))
    }
}

/// The directory a skill or subagent entry is installed from, when its source is an absolute
/// local path.
pub fn local_source(entry: &Spec) -> Option<&str> {
    match entry {
        Spec::Skill { source, .. } | Spec::Subagent { source, .. }
            if Path::new(source).is_absolute() =>
        {
            Some(source)
        }
        _ => None,
    }
}

/// The local source of `entry` when nothing is there any more.
pub fn missing_source<'a>(entry: &'a Spec, probe: &dyn Probe) -> Option<&'a str> {
    local_source(entry).filter(|source| !probe.exists(Path::new(source)))
}

/// The one name a skill or subagent entry holds; none for entries holding several or other kinds.
pub fn sole_name(entry: &Spec) -> Option<&str> {
    match entry {
        Spec::Skill { names, .. } | Spec::Subagent { names, .. } => match names.as_slice() {
            [name] => Some(name),
            _ => None,
        },
        _ => None,
    }
}

/// The local source of a skill entry that exists but no longer holds a skill the entry names.
pub fn source_without_skill<'a>(entry: &'a Spec, probe: &dyn Probe) -> Option<&'a str> {
    let Spec::Skill { names, .. } = entry else {
        return None;
    };
    local_source(entry).filter(|source| {
        names
            .iter()
            .any(|name| !probe.holds_skill(Path::new(source), name))
    })
}

/// The same entry installed from `new_source`.
pub fn with_source(mut entry: Spec, new_source: &str) -> Spec {
    if let Spec::Skill { source, .. } | Spec::Subagent { source, .. } = &mut entry {
        *source = new_source.to_string();
    }
    entry
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use super::*;

    pub struct Existing(pub HashSet<PathBuf>);

    impl Existing {
        pub fn of(paths: &[&str]) -> Self {
            Existing(paths.iter().map(PathBuf::from).collect())
        }
    }

    impl Probe for Existing {
        fn exists(&self, path: &Path) -> bool {
            self.0.contains(path)
        }
    }

    pub fn skill(source: &str, name: &str) -> Spec {
        Spec::Skill {
            source: source.into(),
            names: vec![name.into()],
            agents: vec![],
        }
    }

    #[test]
    fn an_absolute_source_that_is_gone_is_missing() {
        let entry = skill("/old/skills", "mine");

        let missing = missing_source(&entry, &Existing::of(&[]));

        assert_eq!(missing, Some("/old/skills"));
    }

    #[test]
    fn an_absolute_source_that_exists_is_not_missing() {
        let entry = skill("/new/skills", "mine");

        assert_eq!(
            missing_source(&entry, &Existing::of(&["/new/skills"])),
            None
        );
    }

    #[test]
    fn git_and_url_sources_are_never_missing() {
        let nothing = Existing::of(&[]);

        for source in ["org/repo", "https://h/o/r.git", "git@h:o/r.git"] {
            assert_eq!(missing_source(&skill(source, "a"), &nothing), None);
        }
    }

    #[test]
    fn the_disk_holds_the_skills_found_under_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        skillz_harness::test_support::skill(dir.path(), "nested/x");

        assert!(Disk.holds_skill(dir.path(), "x"));
        assert!(!Disk.holds_skill(dir.path(), "y"));
    }

    #[test]
    fn a_folder_that_cannot_be_read_is_not_reported_as_missing_the_skill() {
        assert!(Disk.holds_skill(Path::new("/no/such/folder"), "x"));
    }

    #[test]
    fn only_entries_naming_one_thing_have_a_sole_name() {
        let two = Spec::Skill {
            source: "/p".into(),
            names: vec!["a".into(), "b".into()],
            agents: vec![],
        };

        assert_eq!(sole_name(&skill("/p", "a")), Some("a"));
        assert_eq!(sole_name(&two), None);
    }

    #[test]
    fn changing_the_source_keeps_names_and_agents() {
        let moved = with_source(skill("/old", "a"), "/new");

        assert_eq!(moved, skill("/new", "a"));
    }
}
