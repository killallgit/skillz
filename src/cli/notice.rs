//! What the workspace footer reports after an action, kept typed until the screen is built.

use std::fmt;
use std::path::PathBuf;

use crate::snapshot::Reason;
use crate::tui::home::Act;

pub(super) enum Notice {
    Saved {
        name: String,
        entries: usize,
        skipped: Vec<Reason>,
    },
    Added {
        name: String,
        count: usize,
    },
    Removed {
        name: String,
        count: usize,
    },
    Failed(String),
    UpToDate,
    /// A dry run: nothing changed.
    DryRun(usize),
    Switched {
        project: PathBuf,
        from: Option<String>,
        to: Option<String>,
    },
}

impl Notice {
    /// The request that puts back what a switch replaced.
    pub(super) fn undo(&self) -> Option<Act> {
        let Notice::Switched { project, from, .. } = self else {
            return None;
        };
        let project = project.clone();
        Some(match from {
            Some(profile) => Act::Apply {
                project,
                profile: profile.clone(),
            },
            None => Act::Unload { project },
        })
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Notice::Saved {
                name,
                entries,
                skipped,
            } if skipped.is_empty() => write!(f, "saved {name}: {entries} entries"),
            Notice::Saved {
                name,
                entries,
                skipped,
            } => {
                let why: Vec<String> = distinct(skipped).iter().map(Reason::to_string).collect();
                write!(
                    f,
                    "saved {name}: {entries} entries, {} not reproducible ({})",
                    skipped.len(),
                    why.join("; ")
                )
            }
            Notice::Added { name, count } => write!(f, "added {count} entries to {name}"),
            Notice::Removed { name, count } => write!(f, "removed {count} entries from {name}"),
            Notice::Failed(message) => f.write_str(message),
            Notice::UpToDate => f.write_str("already up to date"),
            Notice::DryRun(steps) => write!(f, "dry run: {steps} steps, nothing changed"),
            Notice::Switched { to: Some(to), .. } => write!(f, "applied {to}"),
            Notice::Switched { to: None, .. } => f.write_str("unloaded"),
        }
    }
}

fn distinct(reasons: &[Reason]) -> Vec<Reason> {
    let mut seen: Vec<Reason> = Vec::new();
    for reason in reasons {
        let same_kind = |r: &Reason| std::mem::discriminant(r) == std::mem::discriminant(reason);
        if !seen.iter().any(same_kind) {
            seen.push(*reason);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undoing_a_switch_reapplies_the_previous_profile_or_unloads() {
        let switched = |from: Option<&str>| Notice::Switched {
            project: "/p".into(),
            from: from.map(String::from),
            to: Some("new".into()),
        };

        let undo = [switched(Some("old")).undo(), switched(None).undo()];

        // Back to the old profile, or to nothing loaded.
        assert_eq!(
            undo,
            [
                Some(Act::Apply {
                    project: "/p".into(),
                    profile: "old".into()
                }),
                Some(Act::Unload {
                    project: "/p".into()
                }),
            ]
        );
    }

    #[test]
    fn each_kind_of_reason_is_listed_once_in_first_seen_order() {
        let reasons = [
            Reason::NoSource,
            Reason::Secrets(1),
            Reason::NoSource,
            Reason::Secrets(3),
        ];

        let seen = distinct(&reasons);

        // Assert the first of each kind is kept.
        assert_eq!(seen, [Reason::NoSource, Reason::Secrets(1)]);
    }
}
