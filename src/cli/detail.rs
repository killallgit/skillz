//! The detail pane's lines for a project and for a profile: entries grouped by kind and tagged,
//! and how a project compares with the profile it has loaded.

use std::path::Path;

use crate::apply::{Loaded, Record};
use crate::profile::{Entry, Profile};
use crate::tui::home::{DetailLine, EntryKind};
use crate::tui::theme::State;

fn kind_of(entry: &Entry) -> EntryKind {
    match entry {
        Entry::Skill { .. } => EntryKind::Skill,
        Entry::Plugin { .. } => EntryKind::Plugin,
        Entry::Subagent { .. } => EntryKind::Subagent,
        Entry::Hook { .. } => EntryKind::Hook,
        Entry::Mcp { .. } => EntryKind::Mcp,
    }
}

/// The entry as written, without the kind word the tag already shows.
fn entry_text(entry: &Entry) -> String {
    let text = entry.to_string();
    match text.split_once(' ') {
        Some((_, rest)) => rest.to_string(),
        None => text,
    }
}

fn entry_line(entry: &Entry) -> DetailLine {
    DetailLine::Entry(kind_of(entry), entry_text(entry))
}

pub(super) fn project(
    path: &Path,
    missing: bool,
    record: Option<&Record>,
    wanted: Option<Vec<Entry>>,
) -> Vec<DetailLine> {
    let mut lines = vec![DetailLine::Heading(path.display().to_string())];
    if missing {
        lines.push(DetailLine::Status(
            State::Danger,
            "directory is missing".into(),
        ));
    }
    let Some(record) = record else {
        return lines;
    };
    lines.push(DetailLine::Text(format!("profile {}", record.profile)));
    lines.push(drift(&record.entries, wanted));
    lines.push(DetailLine::Blank);
    lines.extend(loaded_lines(&record.entries));
    lines
}

/// How the loaded entries differ from the profile as it is now.
fn drift(loaded: &[Loaded], wanted: Option<Vec<Entry>>) -> DetailLine {
    let Some(wanted) = wanted else {
        return DetailLine::Status(State::Danger, "profile is missing or unreadable".into());
    };
    match changes(loaded, &wanted) {
        (0, 0) => DetailLine::Status(State::Ok, "up to date".into()),
        (i, r) => DetailLine::Status(State::Warn, format!("+{i} to install, -{r} to remove")),
    }
}

/// How many wanted entries are not loaded, and how many loaded ones are no longer wanted.
fn changes(loaded: &[Loaded], wanted: &[Entry]) -> (usize, usize) {
    let to_install = wanted
        .iter()
        .filter(|w| !loaded.iter().any(|l| &l.entry == *w))
        .count();
    let to_remove = loaded.iter().filter(|l| !wanted.contains(&l.entry)).count();
    (to_install, to_remove)
}

fn loaded_lines(loaded: &[Loaded]) -> Vec<DetailLine> {
    let mut sorted: Vec<&Loaded> = loaded.iter().collect();
    sorted.sort_by_key(|l| kind_of(&l.entry));
    sorted
        .into_iter()
        .flat_map(|l| {
            let children = l.items.iter().map(|o| DetailLine::Child(o.name.clone()));
            std::iter::once(entry_line(&l.entry)).chain(children)
        })
        .collect()
}

pub(super) fn profile(profile: &Profile) -> Vec<DetailLine> {
    let mut lines = Vec::new();
    if !profile.extends.is_empty() {
        let bases = profile.extends.join(", ");
        lines.push(DetailLine::Text(format!("extends {bases}")));
        lines.push(DetailLine::Blank);
    }
    let mut entries: Vec<&Entry> = profile.entries.iter().collect();
    entries.sort_by_key(|e| kind_of(e));
    lines.extend(entries.into_iter().map(entry_line));
    lines
}

pub(super) fn broken(error: &anyhow::Error) -> Vec<DetailLine> {
    vec![DetailLine::Status(State::Danger, format!("{error:#}"))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Agent;

    fn skill(name: &str) -> Entry {
        Entry::Skill {
            source: "o/r".into(),
            names: vec![name.into()],
            agents: vec![Agent::Claude],
        }
    }

    fn plugin(name: &str) -> Entry {
        Entry::Plugin {
            source: "o/mp".into(),
            marketplace: "mp".into(),
            name: name.into(),
            agents: vec![Agent::Claude],
        }
    }

    fn loaded(entry: Entry) -> Loaded {
        Loaded {
            entry,
            items: vec![],
        }
    }

    #[test]
    fn drift_counts_what_would_be_installed_and_removed() {
        let current = [loaded(skill("a")), loaded(skill("b"))];

        let counts = changes(&current, &[skill("b"), plugin("p"), plugin("q")]);

        // Two plugins are wanted and not loaded; skill a is loaded and no longer wanted.
        assert_eq!(counts, (2, 1));
    }

    #[test]
    fn a_project_matching_its_profile_is_ok_and_a_lost_profile_is_danger() {
        let current = [loaded(skill("a"))];

        let same = drift(&current, Some(vec![skill("a")]));
        let lost = drift(&current, None);

        // Matching reads Ok; an unreadable profile reads Danger.
        let states = [same, lost].map(|l| match l {
            DetailLine::Status(state, _) => Some(state),
            _ => None,
        });
        assert_eq!(states, [Some(State::Ok), Some(State::Danger)]);
    }

    #[test]
    fn entries_are_grouped_by_kind_in_a_fixed_order() {
        let profile = Profile {
            entries: vec![plugin("p"), skill("a")],
            ..Profile::new("x")
        };

        let kinds: Vec<EntryKind> = super::profile(&profile)
            .iter()
            .filter_map(|l| match l {
                DetailLine::Entry(kind, _) => Some(*kind),
                _ => None,
            })
            .collect();

        // Skills come before plugins whatever the file order.
        assert_eq!(kinds, [EntryKind::Skill, EntryKind::Plugin]);
    }

    #[test]
    fn the_kind_word_is_left_to_the_tag() {
        let text = entry_text(&plugin("p"));

        assert!(!text.starts_with("plugin"));
    }
}
