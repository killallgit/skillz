//! How far an installed set is from a profile: what it lacks and what the profile does not ask for.

use super::compare;
use crate::profile::Profile;
use skillz_harness::model::Installed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub profile: String,
    pub missing: usize,
    pub extra: usize,
}

impl Drift {
    pub fn matches(&self) -> bool {
        self.missing == 0 && self.extra == 0
    }
}

pub fn drift(profile: &Profile, installed: &[Installed]) -> Drift {
    let standing = compare(&profile.resolved(), installed);
    Drift {
        profile: profile.name.clone(),
        missing: standing.missing.len(),
        extra: standing.extras.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::Spec;
    use skillz_harness::model::{Agent, Installer, Kind, Scope};

    fn installed(name: &str, kind: Kind, source: Option<&str>) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Project,
            group: source.unwrap_or("local").into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn profile_with(names: &[&str]) -> Profile {
        let mut profile = Profile::new("web");
        profile.entries = names
            .iter()
            .map(|name| Spec::Skill {
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
