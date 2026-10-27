//! The info modal's lines for a package: where it comes from, how it got here and where it lives
//! on disk. Plain data built when the snapshot loads, so drawing never touches the filesystem.

use super::rows::Plugin;
use crate::app::info::describe_installer;
use crate::app::library::{Cue, LibraryItem};
use crate::app::location::location;
use crate::app::package::Package;
use skillz_harness::model::Installed;
use skillz_harness::redact::redact;

const DAY_SECS: u64 = 24 * 60 * 60;

/// Names listed before the rest are counted.
const SHOWN: usize = 12;

/// `plugin` with what the library offers of it, the installed items behind it and the time now
/// (seconds since the epoch).
pub(super) fn lines(
    plugin: &Plugin,
    offered: &[&LibraryItem],
    held: &[&Installed],
    now: u64,
) -> Vec<String> {
    let mut lines = vec![format!("from       {}", origin(&plugin.package))];
    lines.push(status(plugin));
    lines.extend(provenance(offered, now));
    lines.extend(section(
        "offers",
        offered.iter().map(|item| item.entry.name()).collect(),
    ));
    lines.extend(section("installed at", places(held)));
    lines
}

fn origin(package: &Package) -> String {
    match package {
        Package::Marketplace(name) => format!("{name} (plugin marketplace)"),
        other => format!("{} (repository)", redact(&other.title())),
    }
}

fn status(plugin: &Plugin) -> String {
    let tools: Vec<String> = plugin.tools.iter().map(|t| t.to_string()).collect();
    match plugin.is_installed() {
        true => format!("installed  yes, for {}", tools.join(", ")),
        false => "installed  no".into(),
    }
}

/// How the library came to know it: the cue, when, and what it can be fetched from.
fn provenance(offered: &[&LibraryItem], now: u64) -> Vec<String> {
    let recorded = offered.iter().find_map(|item| item.provenance.as_ref());
    let how = match Cue::of(recorded) {
        Cue::Manual => "added by hand",
        Cue::Detected => "found installed by a scan",
        Cue::Skillz => "installed by skillz",
    };
    let Some(recorded) = recorded else {
        return vec![format!("recorded   {how}")];
    };
    let mut lines = vec![format!(
        "recorded   {how}, {}",
        ago(now.saturating_sub(recorded.recorded_at) / DAY_SECS)
    )];
    lines.extend(
        recorded
            .link
            .iter()
            .map(|l| format!("fetch      {}", redact(l))),
    );
    lines.push(format!(
        "with       {}",
        describe_installer(recorded.engine)
    ));
    lines.extend(recorded.version.iter().map(|v| format!("version    {v}")));
    lines
}

fn ago(days: u64) -> String {
    match days {
        0 => "today".into(),
        1 => "1 day ago".into(),
        n => format!("{n} days ago"),
    }
}

/// Each installed item with where its files or config are.
fn places(held: &[&Installed]) -> Vec<String> {
    held.iter()
        .filter_map(|item| {
            let at = location(item);
            (!at.is_empty()).then(|| format!("{}  {at}", item.name))
        })
        .collect()
}

/// A blank line, a heading with the count and the entries, cut to a few with the rest counted;
/// nothing when there are none.
fn section(heading: &str, entries: Vec<String>) -> Vec<String> {
    if entries.is_empty() {
        return entries;
    }
    let mut lines = vec![String::new(), format!("{heading} ({})", entries.len())];
    lines.extend(entries.iter().take(SHOWN).map(|e| format!("  {e}")));
    if entries.len() > SHOWN {
        lines.push(format!("  … {} more", entries.len() - SHOWN));
    }
    lines
}
