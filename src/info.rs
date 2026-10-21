//! The lines of the info modal: what an item or group is, as plain data built before the
//! terminal opens, so drawing never touches the filesystem.

use crate::model::{Augmentation, Installer};

/// Last use of an item, as far as the agents record it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Used {
    /// No agent records uses of this kind of item (hooks, marketplaces).
    NotTracked,
    Never,
    /// Last use, ms since the epoch.
    At(u64),
}

impl Installer {
    pub fn describe(self) -> &'static str {
        match self {
            Installer::Npx => "npx skills",
            Installer::ClaudeCli => "claude plugin",
            Installer::CodexCli => "codex plugin",
            Installer::Local => "by hand (no tool recorded it)",
        }
    }
}

const MAX_CHILDREN: usize = 12;

/// Everything worth knowing about one item.
pub fn item_lines(item: &Augmentation, used: Used, now_ms: u64) -> Vec<String> {
    let agents: Vec<String> = item.agents.iter().map(|a| a.to_string()).collect();
    let mut lines = vec![
        format!("kind       {}", item.kind),
        format!("scope      {}", item.scope),
        format!("agents     {}", agents.join(", ")),
        format!("group      {}", item.group),
    ];
    if let Some(source) = &item.source {
        lines.push(format!("source     {source}"));
    }
    lines.push(format!("installed  {}", item.installer.describe()));
    match used {
        Used::NotTracked => {}
        Used::Never => lines.push("last used  never".into()),
        Used::At(ms) => {
            let days = now_ms.saturating_sub(ms) / (24 * 60 * 60 * 1000);
            lines.push(format!("last used  {days}d ago"));
        }
    }
    if !item.children.is_empty() {
        lines.push(String::new());
        lines.push(format!("contains ({})", item.children.len()));
        lines.extend(
            item.children
                .iter()
                .take(MAX_CHILDREN)
                .map(|c| format!("  {c}")),
        );
        if item.children.len() > MAX_CHILDREN {
            lines.push(format!("  … {} more", item.children.len() - MAX_CHILDREN));
        }
    }
    if !item.removal.is_empty() {
        lines.push(String::new());
        lines.push("removing it runs".into());
        lines.extend(item.removal.iter().map(|a| format!("  {}", a.describe())));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::model::{Agent, Kind, Scope};

    fn item() -> Augmentation {
        Augmentation {
            id: "x".into(),
            name: "x".into(),
            kind: Kind::Plugin,
            agents: vec![Agent::Claude, Agent::Codex],
            scope: Scope::Global,
            group: "mp".into(),
            source: Some("org/repo".into()),
            installer: Installer::ClaudeCli,
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

    #[test]
    fn lines_cover_agents_source_children_and_the_removal_plan() {
        let text = item_lines(&item(), Used::At(0), 3 * 24 * 60 * 60 * 1000).join("\n");
        for needle in [
            "claude, codex",
            "org/repo",
            "claude plugin",
            "3d ago",
            "… 8 more",
            "uninstall x",
        ] {
            assert!(text.contains(needle), "{needle}");
        }
    }

    #[test]
    fn use_line_only_for_tracked_items() {
        let has = |u| {
            item_lines(&item(), u, 0)
                .iter()
                .any(|l| l.contains("last used"))
        };
        assert!(!has(Used::NotTracked));
        assert!(has(Used::Never) && has(Used::At(0)));
    }
}
