//! `skillz sync`: add what is installed to the library. It never removes anything from it.

use std::io::Write;

use serde_json::{Value, json};

use super::scope::Scene;
use crate::library::{LibraryItem, Moved, Synced};
use crate::snapshot::Skipped;
use crate::stale::Disk;
use crate::store::Store;
use crate::sync::sync_library;

pub(super) struct SyncRequest {
    pub dry_run: bool,
    pub json: bool,
}

pub(super) fn sync(
    scene: &Scene,
    request: &SyncRequest,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let items = scene.gather();
    let report = sync_library(&Store::in_env(scene.env), &items, (request.dry_run, &Disk))?;
    if request.json {
        let value = as_json(&report.synced, request.dry_run);
        return Ok(writeln!(out, "{value:#}")?);
    }
    print_synced(out, &report.synced, request.dry_run)
}

fn as_json(synced: &Synced, dry_run: bool) -> Value {
    json!({
        "dry_run": dry_run,
        "added": synced.added.iter().map(item_json).collect::<Vec<_>>(),
        "skipped": synced.skipped.iter().map(skipped_json).collect::<Vec<_>>(),
        "repointed": synced.repointed.iter().map(moved_json).collect::<Vec<_>>(),
        "ambiguous": synced.ambiguous,
    })
}

fn item_json(item: &LibraryItem) -> Value {
    let provenance = item.provenance.as_ref();
    json!({
        "label": item.label,
        "kind": format!("{:?}", item.kind()).to_lowercase(),
        "link": provenance.and_then(|p| p.link.clone()),
        "engine": provenance.map(|p| p.engine),
        "version": provenance.and_then(|p| p.version.clone()),
    })
}

fn moved_json(moved: &Moved) -> Value {
    json!({
        "kind": format!("{:?}", moved.kind).to_lowercase(),
        "name": moved.name,
        "from": moved.from,
        "to": moved.to,
        "profiles": moved.profiles,
    })
}

fn skipped_json(skipped: &Skipped) -> Value {
    json!({ "label": skipped.label, "reason": skipped.reason.to_string() })
}

fn print_synced(out: &mut dyn Write, synced: &Synced, dry_run: bool) -> anyhow::Result<()> {
    let verb = if dry_run { "would add" } else { "added" };
    for item in &synced.added {
        writeln!(out, "{verb} {}", item.label)?;
    }
    for skipped in &synced.skipped {
        writeln!(out, "skipped {} ({})", skipped.label, skipped.reason)?;
    }
    for moved in &synced.repointed {
        writeln!(out, "moved {}: {} -> {}", moved.name, moved.from, moved.to)?;
    }
    for label in &synced.ambiguous {
        writeln!(out, "stale {label}: several folders match, left as it is")?;
    }
    writeln!(out, "{verb} {} to the library", synced.added.len())?;
    writeln!(out, "moved: {}", synced.repointed.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Entry;

    #[test]
    fn the_json_lists_added_items_with_their_kind_and_the_dry_run_flag() {
        let hook = LibraryItem::from_entry(Entry::Hook {
            event: "Stop".into(),
            matcher: None,
            command: "fmt".into(),
            agents: vec![],
        });
        let synced = Synced {
            added: vec![hook],
            ..Synced::default()
        };

        let value = as_json(&synced, true);

        assert_eq!(value["dry_run"], true);
        assert_eq!(value["added"][0]["kind"], "hook");
        assert!(value["skipped"].as_array().unwrap().is_empty());
    }
}
