use std::fmt;

use crate::app::stale::{Probe, missing_source, source_without_skill};
use skillz_harness::Spec;

/// An entry cannot be installed because the local folder it names is gone, or no longer holds
/// the skill, most likely moved.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceGone {
    pub entry: String,
    pub path: String,
}

impl SourceGone {
    /// Why `entry` cannot be installed, when its local source is gone.
    pub fn of(entry: &Spec, probe: &dyn Probe) -> Option<Self> {
        missing_source(entry, probe)
            .or_else(|| source_without_skill(entry, probe))
            .map(|path| SourceGone {
                entry: entry.to_string(),
                path: path.to_string(),
            })
    }
}

impl fmt::Display for SourceGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} cannot be installed: {} no longer holds it; if the folder moved, run `skillz sync` to follow it",
            self.entry, self.path
        )
    }
}

impl std::error::Error for SourceGone {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::*;
    use crate::app::stale::tests::skill;

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

        assert_eq!(gone.map(|g| g.path), Some("/d".to_string()),);
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
