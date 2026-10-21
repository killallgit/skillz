//! A profile: a named set of things to load into a project, kept as a toml file.
//!
//! Pure data plus a directory store. Turning entries into install steps is `provision`.

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::model::Agent;

/// What a profile loads. A new kind of thing = a new variant here, a branch in
/// `Entry::validate`, and a `provision` arm in each adapter that can take it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// Skills of one `org/repo` (or git URL, or local path), installed with `npx skills`.
    Skill {
        source: String,
        names: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
    /// One plugin of a marketplace; `source` is what adds the marketplace and `marketplace` the
    /// name the agent files it under.
    Plugin {
        source: String,
        marketplace: String,
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<Agent>,
    },
}

impl Entry {
    /// The entry's own agents; empty means "the profile's".
    pub fn agents(&self) -> &[Agent] {
        match self {
            Entry::Skill { agents, .. } | Entry::Plugin { agents, .. } => agents,
        }
    }

    /// Every value here ends up as a command-line argument, so none may be empty or option-like.
    pub fn validate(&self) -> anyhow::Result<()> {
        let args: Vec<&str> = match self {
            Entry::Skill { source, names, .. } => {
                anyhow::ensure!(
                    !names.is_empty(),
                    "skill entry for {source} names no skills"
                );
                std::iter::once(source.as_str())
                    .chain(names.iter().map(String::as_str))
                    .collect()
            }
            Entry::Plugin {
                source,
                marketplace,
                name,
                ..
            } => vec![source, marketplace, name],
        };
        for arg in args {
            anyhow::ensure!(
                !arg.trim().is_empty() && !arg.starts_with('-'),
                "invalid value {arg:?} in {self}"
            );
        }
        Ok(())
    }
}

impl std::fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Skill { source, names, .. } => {
                write!(f, "skills {} [{source}]", names.join(","))
            }
            Entry::Plugin {
                marketplace, name, ..
            } => write!(f, "plugin {name}@{marketplace}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// The file stem; not stored in the file.
    #[serde(skip)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Agents an entry applies to unless it names its own.
    #[serde(default = "default_agents")]
    pub agents: Vec<Agent>,
    #[serde(default, rename = "entry")]
    pub entries: Vec<Entry>,
}

fn default_agents() -> Vec<Agent> {
    vec![Agent::Claude, Agent::Codex]
}

impl Profile {
    pub fn new(name: &str) -> Self {
        Profile {
            name: name.into(),
            description: None,
            agents: default_agents(),
            entries: vec![],
        }
    }

    /// The agents `entry` applies to: its own, else the profile's.
    pub fn agents_for<'a>(&'a self, entry: &'a Entry) -> &'a [Agent] {
        match entry.agents() {
            [] => &self.agents,
            own => own,
        }
    }

    /// The entries with every agent list filled in, so two entries compare equal exactly when
    /// they would install the same things.
    pub fn resolved(&self) -> Vec<Entry> {
        self.entries
            .iter()
            .map(|e| {
                let resolved = self.agents_for(e).to_vec();
                let mut e = e.clone();
                let (Entry::Skill { agents, .. } | Entry::Plugin { agents, .. }) = &mut e;
                *agents = resolved;
                e
            })
            .collect()
    }

    pub fn parse(name: &str, text: &str) -> anyhow::Result<Profile> {
        check_name(name)?;
        let mut profile: Profile = toml_edit::de::from_str(text).context("invalid profile")?;
        profile.name = name.into();
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
        self.entries.iter().try_for_each(Entry::validate)
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

/// A directory of `<name>.toml` profiles.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    /// `<skillz dir>/profiles`.
    pub fn in_env(env: &crate::env::Env) -> Self {
        Store::new(env.skillz_dir().join("profiles"))
    }

    fn path(&self, name: &str) -> anyhow::Result<PathBuf> {
        check_name(name)?;
        Ok(self.dir.join(format!("{name}.toml")))
    }

    /// Profile names, sorted. A missing directory has none.
    pub fn names(&self) -> anyhow::Result<Vec<String>> {
        let Ok(read) = fs::read_dir(&self.dir) else {
            return Ok(vec![]);
        };
        let mut names: Vec<String> = read
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                (path.extension()? == "toml")
                    .then(|| path.file_stem()?.to_str().map(String::from))
                    .flatten()
            })
            .filter(|n| check_name(n).is_ok())
            .collect();
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> anyhow::Result<Profile> {
        let path = self.path(name)?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Profile::parse(name, &text).with_context(|| path.display().to_string())
    }

    pub fn save(&self, profile: &Profile) -> anyhow::Result<()> {
        profile.validate()?;
        let path = self.path(&profile.name)?;
        fs::create_dir_all(&self.dir)?;
        fs::write(&path, profile.to_text()?).with_context(|| format!("writing {}", path.display()))
    }

    /// Missing is fine.
    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        match fs::remove_file(self.path(name)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        let mut p = Profile::new("web");
        p.description = Some("frontend".into());
        p.entries = vec![
            Entry::Skill {
                source: "o/r".into(),
                names: vec!["tdd".into(), "grilling".into()],
                agents: vec![],
            },
            Entry::Plugin {
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
        assert_eq!(Profile::parse("web", &p.to_text().unwrap()).unwrap(), p);
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
    fn invalid_entries_are_rejected() {
        let bad = [
            "[[entry]]\nkind = \"skill\"\nsource = \"o/r\"\nnames = []\n",
            "[[entry]]\nkind = \"skill\"\nsource = \"-g\"\nnames = [\"a\"]\n",
            "[[entry]]\nkind = \"skill\"\nsource = \"o/r\"\nnames = [\"--all\"]\n",
            "[[entry]]\nkind = \"plugin\"\nsource = \"o/r\"\nmarketplace = \"\"\nname = \"p\"\n",
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

    #[test]
    fn store_saves_lists_loads_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("profiles"));
        assert!(store.names().unwrap().is_empty());
        let p = sample();
        store.save(&p).unwrap();
        store.save(&Profile::new("a")).unwrap();
        fs::write(dir.path().join("profiles/notes.txt"), "x").unwrap();
        assert_eq!(store.names().unwrap(), ["a", "web"]);
        assert_eq!(store.load("web").unwrap(), p);
        store.delete("web").unwrap();
        store.delete("web").unwrap();
        assert_eq!(store.names().unwrap(), ["a"]);
        assert!(store.load("web").is_err());
    }

    #[test]
    fn saving_an_invalid_profile_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let mut p = sample();
        p.agents.clear();
        assert!(store.save(&p).is_err());
        assert!(store.names().unwrap().is_empty());
    }
}
