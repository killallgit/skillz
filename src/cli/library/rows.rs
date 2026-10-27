//! The library as rows: each item with where it came from and where it is installed.

use serde_json::{Value, json};

use std::collections::BTreeMap;
use std::path::Path;

use crate::app::app_paths::tilde;
use crate::app::library::{Cue, Held, LibraryItem, Presence};
use crate::app::package::Package;
use crate::app::rule::centered_rule;
use crate::app::stale::{Probe, missing_source};
use skillz_harness::redact::redact;

pub(in crate::cli) struct Row<'a> {
    item: &'a LibraryItem,
    presence: Presence,
    /// Its local source is gone: the folder moved or was deleted.
    stale: bool,
}

pub(in crate::cli) fn rows<'a>(
    items: Vec<&'a LibraryItem>,
    (held, probe): (&Held, &dyn Probe),
) -> Vec<Row<'a>> {
    items
        .into_iter()
        .map(|item| Row {
            item,
            presence: held.presence(&item.entry),
            stale: missing_source(&item.entry, probe).is_some(),
        })
        .collect()
}

/// Named packages first by name, then the folders hand-made items live in, MCP servers and
/// hooks; the title is what the rule over the group says.
fn group_of(item: &LibraryItem) -> (u8, String) {
    match Package::of_entry(&item.entry) {
        Package::Source(name) | Package::Marketplace(name) => (0, name),
        Package::Local => (1, item.source()),
        Package::Mcp => (2, Package::Mcp.title()),
        Package::Hooks => (3, Package::Hooks.title()),
    }
}

/// The rows under a rule per package or folder, `~` for the home directory, one line per item.
pub(in crate::cli) fn grouped(rows: &[Row], home: &Path, width: usize) -> Vec<String> {
    let mut groups: BTreeMap<(u8, String), Vec<&Row>> = BTreeMap::new();
    for row in rows {
        let (rank, title) = group_of(row.item);
        groups
            .entry((rank, title.to_lowercase()))
            .or_default()
            .push(row);
    }
    let mut lines = Vec::new();
    for (_, mut members) in groups {
        members.sort_by_key(|row| (row.item.kind(), row.item.short_label().to_lowercase()));
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let title = tilde(&redact(&group_of(members[0].item).1), home);
        lines.push(centered_rule(&title, width, "─"));
        lines.extend(members.iter().map(|row| row.entry_line(home)));
    }
    lines
}

impl Row<'_> {
    /// `tdd  skill  project global  (detected)`: the item inside its package, what it is and where
    /// it is installed.
    fn entry_line(&self, home: &Path) -> String {
        let places = [
            (self.presence.project, "project"),
            (self.presence.global, "global"),
        ]
        .into_iter()
        .filter_map(|(here, name)| here.then_some(name))
        .collect::<Vec<_>>()
        .join(" ");
        let origin = format!("{:?}", Cue::of(self.item.provenance.as_ref())).to_lowercase();
        let places = if places.is_empty() {
            "-".into()
        } else {
            places
        };
        let stale = if self.stale { "  ! missing source" } else { "" };
        let name = tilde(&self.item.short_label(), home);
        format!(
            "  {name:<30} {:<11} {places:<15} ({origin}){stale}",
            self.item.kind().noun()
        )
    }

    pub fn json(&self) -> Value {
        let provenance = self.item.provenance.as_ref();
        json!({
            "label": self.item.label,
            "kind": format!("{:?}", self.item.kind()).to_lowercase(),
            "source": self.item.source(),
            "agents": self.item.entry.agents(),
            "origin": Cue::of(provenance),
            "engine": provenance.map(|p| p.engine),
            "link": provenance.and_then(|p| p.link.clone()),
            "version": provenance.and_then(|p| p.version.clone()),
            "stale": self.stale,
            "installed": {"project": self.presence.project, "global": self.presence.global},
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::stale::Disk;
    use crate::app::stale::tests::{Existing, skill};
    use skillz_harness::Spec;

    fn hook() -> LibraryItem {
        LibraryItem::from_entry(Spec::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "fmt".into(),
            agents: vec![],
        })
    }

    #[test]
    fn an_item_typed_by_hand_is_manual_and_installed_nowhere() {
        let item = hook();
        let held = Held::of(&[], &[]);

        let row = rows(vec![&item], (&held, &Disk)).remove(0).json();

        assert_eq!(row["origin"], "manual");
        assert_eq!(row["kind"], "hook");
        assert_eq!(row["installed"]["project"], false);
        assert!(row["engine"].is_null());
    }

    #[test]
    fn an_item_whose_local_source_is_gone_is_flagged_stale() {
        let moved = LibraryItem::from_entry(skill("/old/skills", "mine"));
        let kept = LibraryItem::from_entry(skill("/new/skills", "mine"));
        let held = Held::of(&[], &[]);
        let probe = Existing::of(&["/new/skills"]);

        let rows = rows(vec![&moved, &kept], (&held, &probe));

        assert_eq!(rows[0].json()["stale"], true);
        assert_eq!(rows[1].json()["stale"], false);
    }

    fn at(path: &str, name: &str) -> LibraryItem {
        LibraryItem::from_entry(skill(path, name))
    }

    #[test]
    fn items_are_grouped_under_a_rule_per_package_with_the_home_directory_as_tilde() {
        let (a, b, c) = (
            at("/home/me/code/skills", "tdd"),
            at("/home/me/code/skills", "grill"),
            at("o/r", "other"),
        );
        let held = Held::of(&[], &[]);
        let rows = rows(vec![&a, &c, &b], (&held, &Disk));

        let lines = grouped(&rows, Path::new("/home/me"), 40);

        let rules: Vec<&String> = lines.iter().filter(|l| l.starts_with('─')).collect();
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.chars().count() == 40));
        assert!(rules.iter().any(|r| r.contains(" ~/code/skills ")));
        assert!(!lines.iter().any(|l| l.contains("/home/me")));
        // The package's two items sit under its rule, `grill` before `tdd`.
        let at_rule = lines
            .iter()
            .position(|l| l.contains("~/code/skills"))
            .unwrap();
        assert!(lines[at_rule + 1].contains("grill") && lines[at_rule + 2].contains("tdd"));
    }

    #[test]
    fn json_keeps_the_paths_as_they_are() {
        let item = at("/home/me/code/skills", "tdd");
        let held = Held::of(&[], &[]);

        let row = rows(vec![&item], (&held, &Disk)).remove(0).json();

        assert_eq!(row["source"], "/home/me/code/skills");
    }
}
