//! Profile entries that named a folder which moved follow it to its new place.

use super::item::ItemKind;
use super::repoint::Moved;
use crate::profile::Profile;
use crate::stale::{local_source, with_source};
use skillz_harness::Spec;

/// Point the entries of `profiles` at the new path of `moved`; the names of the profiles changed.
pub fn repoint_profiles(profiles: &mut [Profile], moved: &Moved) -> Vec<String> {
    profiles
        .iter_mut()
        .filter_map(|profile| {
            let mut changed = false;
            while move_one(&mut profile.entries, moved) {
                changed = true;
            }
            changed.then(|| profile.name.clone())
        })
        .collect()
}

fn names_mut(entry: &mut Spec) -> Option<&mut Vec<String>> {
    match entry {
        Spec::Skill { names, .. } | Spec::Subagent { names, .. } => Some(names),
        _ => None,
    }
}

fn holds_old(entry: &mut Spec, moved: &Moved) -> bool {
    ItemKind::of_entry(entry) == moved.kind
        && local_source(entry) == Some(moved.from.as_str())
        && names_mut(entry).is_some_and(|names| names.contains(&moved.name))
}

/// Take the moved name out of the first entry holding it and put it under the new path.
fn move_one(entries: &mut Vec<Spec>, moved: &Moved) -> bool {
    let Some(at) = entries.iter_mut().position(|e| holds_old(e, moved)) else {
        return false;
    };
    let old = entries[at].clone();
    if let Some(names) = names_mut(&mut entries[at]) {
        names.retain(|n| *n != moved.name);
        if names.is_empty() {
            entries.remove(at);
        }
    }
    let same_home = |e: &Spec| {
        ItemKind::of_entry(e) == moved.kind
            && local_source(e) == Some(moved.to.as_str())
            && e.agents() == old.agents()
    };
    match entries.iter_mut().find(|e| same_home(e)) {
        Some(home) => {
            if let Some(names) = names_mut(home) {
                names.push(moved.name.clone());
            }
        }
        None => entries.push(with_names(with_source(old, &moved.to), &moved.name)),
    }
    true
}

fn with_names(mut entry: Spec, name: &str) -> Spec {
    if let Some(names) = names_mut(&mut entry) {
        *names = vec![name.to_string()];
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stale::tests::skill;

    fn moved(name: &str) -> Moved {
        Moved {
            kind: ItemKind::Skill,
            name: name.into(),
            from: "/old".into(),
            to: "/new".into(),
            profiles: vec![],
        }
    }

    fn profile_with(entries: Vec<Spec>) -> Profile {
        Profile {
            entries,
            ..Profile::new("p")
        }
    }

    #[test]
    fn an_entry_naming_the_old_path_follows_the_move() {
        let mut profiles = [profile_with(vec![skill("/old", "mine")])];

        let changed = repoint_profiles(&mut profiles, &moved("mine"));

        assert_eq!(changed, ["p"]);
        assert_eq!(profiles[0].entries, [skill("/new", "mine")]);
    }

    #[test]
    fn only_the_moved_name_leaves_a_shared_entry() {
        let both = Spec::Skill {
            source: "/old".into(),
            names: vec!["a".into(), "b".into()],
            agents: vec![],
        };
        let mut profiles = [profile_with(vec![both])];

        repoint_profiles(&mut profiles, &moved("a"));

        assert_eq!(
            profiles[0].entries,
            [skill("/old", "b"), skill("/new", "a")]
        );
    }

    #[test]
    fn unrelated_entries_and_profiles_are_left_alone() {
        let mut profiles = [profile_with(vec![skill("o/r", "mine"), skill("/old", "x")])];

        let changed = repoint_profiles(&mut profiles, &moved("mine"));

        assert!(changed.is_empty());
    }

    #[test]
    fn a_name_joins_the_entry_already_at_the_new_path() {
        let mut profiles = [profile_with(vec![skill("/old", "a"), skill("/new", "b")])];

        repoint_profiles(&mut profiles, &moved("a"));

        let Spec::Skill { names, .. } = &profiles[0].entries[0] else {
            panic!("a skill entry");
        };
        assert_eq!(profiles[0].entries.len(), 1);
        assert_eq!(names, &["b", "a"]);
    }
}
