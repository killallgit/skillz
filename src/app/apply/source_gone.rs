use std::fmt;

use crate::app::stale::{Probe, missing_source, source_without_skill};
use skillz_harness::Spec;

/// Why an entry's local source cannot be installed from.
#[derive(Debug, PartialEq, Eq)]
pub enum Cause {
    FolderMissing,
    SkillMissing,
}

/// An entry cannot be installed because the local folder it names is gone, or no longer holds
/// the skill.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceGone {
    pub entry: String,
    pub path: String,
    pub cause: Cause,
}

impl SourceGone {
    /// Why `entry` cannot be installed, when its local source is gone.
    pub fn of(entry: &Spec, probe: &dyn Probe) -> Option<Self> {
        let gone = |path: &str, cause| SourceGone {
            entry: entry.to_string(),
            path: path.to_string(),
            cause,
        };
        missing_source(entry, probe)
            .map(|path| gone(path, Cause::FolderMissing))
            .or_else(|| {
                source_without_skill(entry, probe).map(|path| gone(path, Cause::SkillMissing))
            })
    }
}

impl fmt::Display for SourceGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cause {
            Cause::FolderMissing => write!(
                f,
                "{} cannot be installed: {} does not exist; if the folder moved, run `skillz sync` to follow it",
                self.entry, self.path
            ),
            Cause::SkillMissing => write!(
                f,
                "{} cannot be installed: {} exists but no longer holds it; restore the skill there or point the entry at the folder that has it",
                self.entry, self.path
            ),
        }
    }
}

impl std::error::Error for SourceGone {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::*;
    use crate::app::stale::tests::{Existing, skill};

    /// Every folder exists; each holds only the skills listed for it.
    struct Holding(HashSet<(&'static str, &'static str)>);

    impl Probe for Holding {
        fn exists(&self, _path: &Path) -> bool {
            true
        }

        fn holds_skill(&self, dir: &Path, name: &str) -> bool {
            self.0
                .iter()
                .any(|(d, n)| Path::new(d) == dir && *n == name)
        }
    }

    fn holding(skills: &[(&'static str, &'static str)]) -> Holding {
        Holding(skills.iter().copied().collect())
    }

    #[test]
    fn a_folder_that_no_longer_holds_the_skill_is_gone() {
        let gone = SourceGone::of(&skill("/d", "x"), &holding(&[("/d", "y")]));

        assert_eq!(gone.map(|g| g.cause), Some(Cause::SkillMissing));
    }

    #[test]
    fn a_missing_folder_is_gone_for_that_reason() {
        let gone = SourceGone::of(&skill("/d", "x"), &Existing::of(&[]));

        assert_eq!(gone.map(|g| g.cause), Some(Cause::FolderMissing));
    }

    #[test]
    fn one_named_skill_missing_from_the_folder_is_enough() {
        let entry = Spec::Skill {
            source: "/d".into(),
            names: vec!["x".into(), "y".into()],
            agents: vec![],
        };

        let gone = SourceGone::of(&entry, &holding(&[("/d", "y")]));

        assert_eq!(gone.map(|g| g.cause), Some(Cause::SkillMissing));
    }

    #[test]
    fn a_subagent_entry_is_never_flagged_for_a_missing_skill() {
        let entry = Spec::Subagent {
            source: "/d".into(),
            names: vec!["x".into()],
            agents: vec![],
        };

        assert_eq!(SourceGone::of(&entry, &holding(&[])), None);
    }

    #[test]
    fn a_folder_that_holds_the_skill_is_not_gone() {
        assert_eq!(
            SourceGone::of(&skill("/d", "x"), &holding(&[("/d", "x")])),
            None
        );
    }

    #[test]
    fn git_and_url_sources_are_not_gone_whatever_the_probe_holds() {
        for source in ["org/repo", "https://h/o/r.git"] {
            assert_eq!(SourceGone::of(&skill(source, "x"), &holding(&[])), None);
        }
    }
}
