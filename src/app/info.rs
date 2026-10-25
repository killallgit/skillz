//! The lines of the info modal: what an item or group is, as plain data built before the
//! terminal opens, so drawing never touches the filesystem.

use skillz_harness::model::{Installed, Installer};

/// Last use of an item, as far as the agents record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Used {
    /// No agent records uses of this kind of item (hooks, marketplaces).
    NotTracked,
    Never,
    /// Last use, ms since the epoch.
    At(u64),
}

fn describe_installer(installer: Installer) -> &'static str {
    match installer {
        Installer::Npx => "npx skills",
        Installer::ClaudeCli => "claude plugin",
        Installer::CodexCli => "codex plugin",
        Installer::Local => "by hand (no tool recorded it)",
    }
}

const MAX_CHILDREN: usize = 12;

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// Everything worth knowing about one item: facts, then what it contains, then what removing it
/// runs; the later sections are left out when empty.
pub fn item_lines(item: &Installed, used: Used, now_ms: u64) -> Vec<String> {
    let mut lines = fact_lines(item);
    lines.extend(use_line(used, now_ms));
    lines.extend(section(contents(item)));
    lines.extend(section(removal(item)));
    lines
}

fn fact_lines(item: &Installed) -> Vec<String> {
    let agents: Vec<String> = item.agents.iter().map(|a| a.to_string()).collect();
    let mut lines = vec![
        format!("kind       {}", item.kind),
        format!("scope      {}", item.scope),
        format!("agents     {}", agents.join(", ")),
        format!("group      {}", item.group),
    ];
    lines.extend(item.source.iter().map(|s| format!("source     {s}")));
    lines.push(format!("installed  {}", describe_installer(item.installer)));
    lines
}

fn use_line(used: Used, now_ms: u64) -> Option<String> {
    match used {
        Used::NotTracked => None,
        Used::Never => Some("last used  never".into()),
        Used::At(ms) => Some(format!(
            "last used  {}d ago",
            now_ms.saturating_sub(ms) / DAY_MS
        )),
    }
}

fn contents(item: &Installed) -> Vec<String> {
    let children = skillz_harness::mcp::shown_children(item);
    if children.is_empty() {
        return vec![];
    }
    let mut lines = vec![format!("contains ({})", children.len())];
    lines.extend(children.iter().take(MAX_CHILDREN).map(|c| format!("  {c}")));
    if children.len() > MAX_CHILDREN {
        lines.push(format!("  … {} more", children.len() - MAX_CHILDREN));
    }
    lines
}

fn removal(item: &Installed) -> Vec<String> {
    if item.removal.is_empty() {
        return vec![];
    }
    let steps = item.removal.iter().map(|a| format!("  {}", a.describe()));
    std::iter::once("removing it runs".to_string())
        .chain(steps)
        .collect()
}

/// A blank line, then `lines`; nothing when there are none.
fn section(lines: Vec<String>) -> Vec<String> {
    if lines.is_empty() {
        return lines;
    }
    std::iter::once(String::new()).chain(lines).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skillz_harness::action::Action;
    use skillz_harness::model::{Agent, Kind, Scope};

    fn item() -> Installed {
        Installed {
            details: Default::default(),
            hooks: Vec::new(),
            members: Vec::new(),
            id: "x".into(),
            name: "x".into(),
            kind: Kind::Plugin,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            group: "mp".into(),
            source: Some("org/repo".into()),
            installer: Installer::ClaudeCli,
            origin: Default::default(),
            children: (0..20).map(|i| format!("skill: s{i}")).collect(),
            removal: vec![
                Action::native(
                    "claude",
                    &["plugin", "uninstall", "x"],
                    &["x"],
                    "/h".as_ref(),
                )
                .unwrap(),
            ],
        }
    }

    fn count(item: &Installed, used: Used) -> usize {
        item_lines(item, used, 0).len()
    }

    #[test]
    fn long_child_lists_are_cut_to_the_cap_with_a_remainder_line() {
        let mut few = item();
        few.children.truncate(MAX_CHILDREN);

        let (all, capped) = (
            count(&item(), Used::NotTracked),
            count(&few, Used::NotTracked),
        );

        // 20 children show MAX_CHILDREN of them plus one "more" line; exactly MAX shows no such line.
        assert_eq!(all, capped + 1);
    }

    #[test]
    fn empty_sections_leave_no_blank_lines() {
        let mut bare = item();
        bare.children.clear();
        bare.removal.clear();

        let lines = item_lines(&bare, Used::NotTracked, 0);

        // Only the facts remain: no blank separator and no section headers.
        assert!(lines.iter().all(|l| !l.is_empty()));
        assert_eq!(lines.len(), fact_lines(&bare).len());
    }

    #[test]
    fn use_line_only_for_tracked_items() {
        let base = count(&item(), Used::NotTracked);

        let (never, at) = (count(&item(), Used::Never), count(&item(), Used::At(0)));

        // Untracked items add no line; tracked ones add exactly one.
        assert_eq!((never, at), (base + 1, base + 1));
    }

    #[test]
    fn days_since_last_use_are_whole_days() {
        let at = |now| use_line(Used::At(0), now);

        let (whole, part, next) = (at(3 * DAY_MS), at(3 * DAY_MS + DAY_MS / 2), at(4 * DAY_MS));

        // Three and a half days read the same as three, and differ from four.
        assert_eq!(whole, part);
        assert_ne!(part, next);
    }
}
