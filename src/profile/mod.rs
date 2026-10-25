//! A profile: a named set of things to load into a project, kept in the global `store`.
//!
//! Pure data plus a facade over the store. Turning entries into install steps is `provision`.

mod edit;

mod identity;
mod spec;
mod store;
mod validate;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::harness::Spec;
use crate::model::Agent;

pub use edit::with_added;

pub use identity::{Claim, claims, lacking};
pub(crate) use identity::{Key, installed_claims, installed_keys, keys, same_thing};
pub use store::{StillExtended, Store};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Profiles whose entries this one starts from, loaded in order before its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<String>,
    /// Agents an entry applies to unless it names its own.
    #[serde(default = "default_agents")]
    pub agents: Vec<Agent>,
    #[serde(default, rename = "entry")]
    pub entries: Vec<Spec>,
}

fn default_agents() -> Vec<Agent> {
    vec![Agent::Claude, Agent::Codex]
}

impl Profile {
    pub fn new(name: &str) -> Self {
        Profile {
            name: name.into(),
            description: None,
            extends: vec![],
            agents: default_agents(),
            entries: vec![],
        }
    }

    /// A temporary profile for comparison and installation; the stored definition stays portable.
    pub fn in_env(mut self, env: &crate::env::Env) -> Self {
        self.entries = self
            .entries
            .into_iter()
            .map(|entry| crate::app_paths::spec_in_env(env, entry))
            .collect();
        self
    }

    /// The agents `entry` applies to: its own, else the profile's.
    pub fn agents_for<'a>(&'a self, entry: &'a Spec) -> &'a [Agent] {
        match entry.agents() {
            [] => &self.agents,
            own => own,
        }
    }

    /// The entries with every agent list filled in, so two entries compare equal exactly when
    /// they would install the same things.
    pub fn resolved(&self) -> Vec<Spec> {
        self.entries
            .iter()
            .map(|e| e.clone().with_agents(self.agents_for(e).to_vec()))
            .collect()
    }

    pub fn parse(name: &str, text: &str) -> anyhow::Result<Profile> {
        check_name(name)?;
        let mut profile: Profile = toml_edit::de::from_str(text).context("invalid profile")?;
        profile.name = name.into();
        profile.validate()?;
        Ok(profile)
    }

    /// A profile as `to_text` wrote it, named by its own `name`.
    pub fn from_text(text: &str) -> anyhow::Result<Profile> {
        let profile: Profile = toml_edit::de::from_str(text).context("invalid profile")?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn to_text(&self) -> anyhow::Result<String> {
        toml_edit::ser::to_string_pretty(self).context("writing profile")
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        check_name(&self.name)?;
        anyhow::ensure!(
            !self.agents.is_empty(),
            "profile {} has no agents",
            self.name
        );
        self.extends.iter().try_for_each(|base| check_name(base))?;
        self.entries.iter().try_for_each(|entry| {
            entry.validate()?;
            entry.ensure_no_credentials()
        })
    }
}

/// A name is a file stem and a command-line argument: no path parts, no leading `-` or `.`.
pub fn check_name(name: &str) -> anyhow::Result<()> {
    let ok = !name.is_empty()
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    anyhow::ensure!(ok, "invalid profile name {name:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        let mut p = Profile::new("web");
        p.description = Some("frontend".into());
        p.entries = vec![
            Spec::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into(), "grilling".into()],
                agents: vec![],
            },
            Spec::Plugin {
                source: "o/mp".into(),
                marketplace: "mp".into(),
                name: "ponytail".into(),
                agents: vec![Agent::Claude],
            },
        ];
        p
    }

    #[test]
    fn text_round_trips() {
        let p = sample();

        let back = Profile::parse("web", &p.to_text().unwrap()).unwrap();

        assert_eq!(back, p);
    }

    #[test]
    fn omitted_fields_take_defaults() {
        let p = Profile::parse("x", "").unwrap();

        assert_eq!(p, Profile::new("x"));
    }

    #[test]
    fn entries_use_their_own_agents_else_the_profiles() {
        let p = sample();

        assert_eq!(p.agents_for(&p.entries[0]), [Agent::Claude, Agent::Codex]);
        assert_eq!(p.agents_for(&p.entries[1]), [Agent::Claude]);
    }

    #[test]
    fn resolving_fills_every_entrys_agents() {
        let resolved = sample().resolved();

        assert_eq!(resolved[0].agents(), [Agent::Claude, Agent::Codex]);
        assert_eq!(resolved[1].agents(), [Agent::Claude]);
    }

    #[test]
    fn a_profile_with_a_bad_entry_or_no_agents_is_rejected() {
        let bad = [
            "[[entry]]\nkind = \"skill\"\nsource = \"o/r\"\nnames = []\n",
            "[[entry]]\nkind = \"nope\"\n",
            "agents = []\n",
        ];

        for text in bad {
            assert!(Profile::parse("x", text).is_err(), "{text}");
        }
    }

    #[test]
    fn names_cannot_escape_the_store_or_look_like_options() {
        for name in ["", "-x", ".hidden", "a/b", "a b", ".."] {
            assert!(check_name(name).is_err(), "{name:?}");
        }
        assert!(check_name("web-1_a.b").is_ok());
    }
}
