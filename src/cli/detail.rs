//! The detail pane's lines for a project and for a profile: entries grouped by kind and tagged,
//! and how a project compares with the profile it has loaded.

use std::path::Path;

use crate::apply::Standing;
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

/// The project's path, and the profile it matches with the entries that are in place.
pub(super) fn project(path: &Path, loaded: Option<(&Profile, &Standing)>) -> Vec<DetailLine> {
    let mut lines = vec![DetailLine::Heading(path.display().to_string())];
    let Some((profile, standing)) = loaded else {
        lines.push(DetailLine::Text("no profile loaded".into()));
        return lines;
    };
    lines.push(DetailLine::Text(format!("profile {}", profile.name)));
    lines.push(DetailLine::Blank);
    lines.extend(entry_lines(&standing.present));
    lines
}

fn entry_lines(entries: &[Entry]) -> Vec<DetailLine> {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by_key(|e| kind_of(e));
    sorted.into_iter().map(entry_line).collect()
}

pub(super) fn profile(profile: &Profile) -> Vec<DetailLine> {
    let mut lines = Vec::new();
    if !profile.extends.is_empty() {
        let bases = profile.extends.join(", ");
        lines.push(DetailLine::Text(format!("extends {bases}")));
        lines.push(DetailLine::Blank);
    }
    lines.extend(entry_lines(&profile.entries));
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

    #[test]
    fn a_project_without_a_matching_profile_says_so() {
        let lines = project(Path::new("/p"), None);

        assert!(matches!(lines.last(), Some(DetailLine::Text(_))));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn a_matching_project_lists_the_entries_in_place() {
        let standing = Standing {
            present: vec![plugin("p"), skill("a")],
            missing: vec![],
            extras: vec![],
        };

        let lines = project(Path::new("/p"), Some((&Profile::new("web"), &standing)));

        let entries = lines
            .iter()
            .filter(|l| matches!(l, DetailLine::Entry(..)))
            .count();
        assert_eq!(entries, 2);
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
