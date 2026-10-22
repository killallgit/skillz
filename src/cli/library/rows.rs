//! The library as rows: each item with where it came from and where it is installed.

use serde_json::{Value, json};

use crate::library::{Cue, Held, LibraryItem, Presence};

pub(in crate::cli) struct Row<'a> {
    item: &'a LibraryItem,
    presence: Presence,
}

pub(in crate::cli) fn rows<'a>(items: Vec<&'a LibraryItem>, held: &Held) -> Vec<Row<'a>> {
    items
        .into_iter()
        .map(|item| Row {
            item,
            presence: held.presence(&item.entry),
        })
        .collect()
}

impl Row<'_> {
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
            "installed": {"project": self.presence.project, "global": self.presence.global},
        })
    }

    pub fn line(&self) -> String {
        let places = [
            (self.presence.project, "project"),
            (self.presence.global, "global"),
        ]
        .into_iter()
        .filter_map(|(here, name)| here.then_some(name))
        .collect::<Vec<_>>()
        .join(" ");
        let origin = format!("{:?}", Cue::of(self.item.provenance.as_ref())).to_lowercase();
        format!("{}  ({origin}) {places}", self.item.label)
            .trim_end()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Entry;

    fn hook() -> LibraryItem {
        LibraryItem::from_entry(Entry::Hook {
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

        let row = rows(vec![&item], &held).remove(0).json();

        assert_eq!(row["origin"], "manual");
        assert_eq!(row["kind"], "hook");
        assert_eq!(row["installed"]["project"], false);
        assert!(row["engine"].is_null());
    }
}
