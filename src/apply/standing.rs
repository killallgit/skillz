//! What a directory already has of a profile, read from its inventory. Pure: nothing here reads
//! or changes the machine.

use std::collections::HashSet;

use crate::model::{Installed, Kind};
use crate::profile::{Claim, Profile, Spec, claims, installed_claims, lacking};
use crate::snapshot;

/// How a profile's entries and the installed items overlap.
#[derive(Debug)]
pub struct Standing {
    /// Wanted entries with everything in place.
    pub present: Vec<Spec>,
    /// What is still to install: the wanted entries cut down to the parts not in place.
    pub missing: Vec<Spec>,
    /// Installed items the profile does not want.
    pub extras: Vec<Extra>,
}

#[derive(Debug)]
pub struct Extra {
    pub item: Installed,
    /// Whether a profile could reinstall this item.
    pub sourced: bool,
}

impl Standing {
    pub fn matches(&self) -> bool {
        self.missing.is_empty()
    }
}

/// The profile an installed set matches: the one with the most entries in place, the first
/// of them on a tie. An empty profile matches everything, so it never counts as loaded.
pub fn loaded<'a>(
    profiles: &'a [Profile],
    installed: &[Installed],
) -> Option<(&'a Profile, Standing)> {
    let mut best: Option<(&Profile, Standing)> = None;
    for profile in profiles.iter().filter(|p| !p.entries.is_empty()) {
        let standing = compare(&profile.resolved(), installed);
        let better = best
            .as_ref()
            .is_none_or(|(_, held)| in_place(&standing) > in_place(held));
        if standing.matches() && better {
            best = Some((profile, standing));
        }
    }
    best
}

fn in_place(standing: &Standing) -> usize {
    standing.present.iter().flat_map(claims).count()
}

pub fn compare(wanted: &[Spec], installed: &[Installed]) -> Standing {
    let held: Vec<(&Installed, Vec<Claim>)> = installed.iter().map(|i| (i, claims_of(i))).collect();
    let have: HashSet<Claim> = held.iter().flat_map(|(_, c)| c.iter().cloned()).collect();
    let wanted = distinct(wanted);
    let wanted_claims: HashSet<Claim> = wanted.iter().flat_map(claims).collect();
    let (mut present, mut missing) = (vec![], vec![]);
    for entry in &wanted {
        match lacking(entry, &have) {
            Some(part) => missing.push(part),
            None => present.push(entry.clone()),
        }
    }
    let extras = held
        .into_iter()
        .filter_map(|(item, held)| extra(item, &held, &wanted_claims, &wanted))
        .collect();
    Standing {
        present,
        missing,
        extras,
    }
}

/// Whether everything `entry` puts in place is in place among `installed`.
pub fn holds(installed: &[Installed], entry: &Spec) -> bool {
    let mut profile = Profile::new("holds");
    profile.entries = vec![entry.clone()];
    compare(&profile.resolved(), installed).missing.is_empty()
}

/// The ids of installed items that `dropped` wants and `kept` does not: what is left over once
/// only the kept entries are to stay.
pub fn only_wanted_by(dropped: &[Spec], kept: &[Spec], installed: &[Installed]) -> Vec<String> {
    let unwanted_by = |entries: &[Spec]| -> HashSet<String> {
        compare(entries, installed)
            .extras
            .into_iter()
            .map(|extra| extra.item.id)
            .collect()
    };
    let (not_dropped, not_kept) = (unwanted_by(dropped), unwanted_by(kept));
    installed
        .iter()
        .filter(|item| !not_dropped.contains(&item.id) && not_kept.contains(&item.id))
        .map(|item| item.id.clone())
        .collect()
}

fn distinct(entries: &[Spec]) -> Vec<Spec> {
    let mut seen: Vec<Spec> = Vec::new();
    for entry in entries {
        if !seen.contains(entry) {
            seen.push(entry.clone());
        }
    }
    seen
}

/// What an installed item puts in place, as a profile would have to claim it.
fn claims_of(item: &Installed) -> Vec<Claim> {
    item.parts().iter().flat_map(installed_claims).collect()
}

/// An item is wanted when any one of its claims is, so one shared with a wanted entry stays.
/// A marketplace is wanted while a wanted plugin comes from it.
fn extra(
    item: &Installed,
    held: &[Claim],
    wanted_claims: &HashSet<Claim>,
    wanted: &[Spec],
) -> Option<Extra> {
    if item.kind == Kind::Marketplace {
        let used = wanted.iter().any(|e| draws_on(e, &item.name));
        return (!used).then(|| Extra {
            item: item.clone(),
            sourced: true,
        });
    }
    if held.iter().any(|c| wanted_claims.contains(c)) {
        return None;
    }
    Some(Extra {
        item: item.clone(),
        sourced: item
            .parts()
            .iter()
            .all(|part| snapshot::entries(part).is_ok_and(|entries| !entries.is_empty())),
    })
}

fn draws_on(entry: &Spec, marketplace: &str) -> bool {
    matches!(entry, Spec::Plugin { marketplace: m, .. } if m == marketplace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, Installer, Scope};

    fn skill_entry(names: &[&str]) -> Spec {
        Spec::Skill {
            source: "o/r".into(),
            names: names.iter().map(|n| n.to_string()).collect(),
            agents: vec![Agent::Claude],
        }
    }

    fn installed(name: &str, kind: Kind, source: Option<&str>) -> Installed {
        Installed {
            details: Default::default(),
            hooks: vec![],
            members: Vec::new(),
            id: format!("{kind}:{name}"),
            name: name.into(),
            kind,
            agents: vec![Agent::Claude],
            scope: Scope::Project,
            group: "g".into(),
            source: source.map(Into::into),
            installer: Installer::Npx,
            origin: Default::default(),
            children: vec![],
            removal: vec![],
        }
    }

    fn sourced_skill(name: &str) -> Installed {
        installed(name, Kind::Skill, Some("o/r"))
    }

    #[test]
    fn a_project_with_everything_wanted_matches_and_extras_do_not_matter() {
        let live = [sourced_skill("a"), sourced_skill("b")];

        let standing = compare(&[skill_entry(&["a"])], &live);

        assert!(standing.matches());
        assert_eq!(standing.present, [skill_entry(&["a"])]);
        assert_eq!(standing.extras.len(), 1);
    }

    #[test]
    fn only_the_names_not_installed_are_missing() {
        let live = [sourced_skill("a")];

        let standing = compare(&[skill_entry(&["a", "b"])], &live);

        assert!(!standing.matches());
        assert_eq!(standing.missing, [skill_entry(&["b"])]);
        assert!(standing.present.is_empty());
    }

    #[test]
    fn a_hand_made_skill_is_an_extra_that_cannot_be_reinstalled() {
        let live = [installed("mine", Kind::Skill, None)];

        let standing = compare(&[], &live);

        assert!(!standing.extras[0].sourced);
    }

    #[test]
    fn an_installed_item_shared_with_a_wanted_entry_is_not_an_extra() {
        let mut both = sourced_skill("a");
        both.agents = vec![Agent::Claude, Agent::Codex];

        let standing = compare(&[skill_entry(&["a"])], &[both]);

        assert!(standing.extras.is_empty());
    }

    #[test]
    fn a_marketplace_stays_while_a_wanted_plugin_comes_from_it() {
        let plugin = Spec::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: "p".into(),
            agents: vec![Agent::Claude],
        };
        let live = [installed("mp", Kind::Marketplace, Some("o/mp"))];

        let wanting = compare(std::slice::from_ref(&plugin), &live);
        let not_wanting = compare(&[], &live);

        assert!(wanting.extras.is_empty());
        assert_eq!(not_wanting.extras.len(), 1);
    }

    #[test]
    fn dropping_an_entry_removes_only_what_no_kept_entry_wants() {
        let live = [sourced_skill("a"), sourced_skill("b"), sourced_skill("c")];

        let removable = only_wanted_by(&[skill_entry(&["a"])], &[skill_entry(&["b"])], &live);

        assert_eq!(removable, ["skill:a"]);
    }

    #[test]
    fn an_item_a_kept_entry_also_wants_stays() {
        let live = [sourced_skill("a")];

        let removable = only_wanted_by(&[skill_entry(&["a"])], &[skill_entry(&["a"])], &live);

        assert!(removable.is_empty());
    }

    fn profile(name: &str, skills: &[&str]) -> Profile {
        let mut p = Profile::new(name);
        p.entries = vec![skill_entry(skills)];
        p.agents = vec![Agent::Claude];
        p
    }

    #[test]
    fn the_loaded_profile_is_the_matching_one_with_the_most_in_place() {
        let live = [sourced_skill("a"), sourced_skill("b")];
        let profiles = [
            profile("small", &["a"]),
            profile("big", &["a", "b"]),
            profile("other", &["c"]),
            Profile::new("empty"),
        ];

        let found = loaded(&profiles, &live);

        assert_eq!(found.map(|(p, _)| p.name.as_str()), Some("big"));
    }

    #[test]
    fn nothing_is_loaded_when_no_profile_matches() {
        let profiles = [profile("web", &["a"])];

        assert!(loaded(&profiles, &[]).is_none());
    }

    #[test]
    fn a_repeated_entry_counts_once() {
        let standing = compare(&[skill_entry(&["a"]), skill_entry(&["a"])], &[]);

        assert_eq!(standing.missing.len(), 1);
    }
    #[test]
    fn an_installed_server_needing_secrets_is_present_but_cannot_be_reinstalled() {
        let server = Installed {
            details: crate::model::Details::Mcp {
                config: "/config".into(),
                server: crate::mcp::Server {
                    name: "db".into(),
                    launch: crate::mcp::Launch::Stdio {
                        command: "srv".into(),
                        args: vec![],
                    },
                    secrets: vec!["TOKEN".into()],
                },
            },
            ..installed("db", Kind::Mcp, None)
        };
        let wanted = Spec::Mcp {
            name: "db".into(),
            command: Some("srv".into()),
            args: vec![],
            url: None,
            transport: None,
            agents: vec![Agent::Claude],
        };
        let standing = compare(&[wanted], std::slice::from_ref(&server));
        assert!(standing.missing.is_empty());
        assert!(standing.extras.is_empty());
        assert!(!compare(&[], &[server]).extras[0].sourced);
    }
}
