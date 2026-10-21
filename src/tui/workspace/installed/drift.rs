//! How far the project is from a profile: what it lacks and what the profile does not ask for.

use crate::apply::compare;
use crate::model::Augmentation;
use crate::profile::Profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Drift {
    pub profile: String,
    pub missing: usize,
    pub extra: usize,
}

impl Drift {
    pub fn matches(&self) -> bool {
        self.missing == 0 && self.extra == 0
    }
}

pub(super) fn drift(profile: &Profile, installed: &[Augmentation]) -> Drift {
    let standing = compare(&profile.resolved(), installed);
    Drift {
        profile: profile.name.clone(),
        missing: standing.missing.len(),
        extra: standing.extras.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::rows::tests::installed;
    use super::*;
    use crate::model::Kind;
    use crate::profile::Entry;

    fn profile_with(names: &[&str]) -> Profile {
        let mut profile = Profile::new("web");
        profile.entries = names
            .iter()
            .map(|name| Entry::Skill {
                source: "o/r".into(),
                names: vec![name.to_string()],
                agents: vec![],
            })
            .collect();
        profile
    }

    #[test]
    fn a_project_with_exactly_the_profile_matches() {
        let have = [installed("tdd", Kind::Skill, Some("o/r"))];

        let drift = drift(&profile_with(&["tdd"]), &have);

        assert!(drift.matches());
    }

    #[test]
    fn what_the_project_lacks_and_what_it_has_extra_are_counted() {
        let have = [
            installed("tdd", Kind::Skill, Some("o/r")),
            installed("other", Kind::Skill, Some("o/r")),
        ];

        let drift = drift(&profile_with(&["tdd", "grill"]), &have);

        assert_eq!((drift.missing, drift.extra), (1, 1));
        assert!(!drift.matches());
    }
}
