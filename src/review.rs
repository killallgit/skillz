//! What a switch shows the user before, while and after it runs, as typed data. Screens decide
//! how to draw it; flows decide how to build it.

use crate::model::Agent;
use crate::profile::Spec;

/// The kinds of line in a plan, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Keep,
    Remove,
    Install,
    Update,
    Skip,
}

impl Group {
    pub fn name(self) -> &'static str {
        match self {
            Group::Keep => "keep",
            Group::Remove => "remove",
            Group::Install => "install",
            Group::Update => "update",
            Group::Skip => "skip",
        }
    }

    pub const ALL: [Group; 5] = [
        Group::Keep,
        Group::Remove,
        Group::Install,
        Group::Update,
        Group::Skip,
    ];
}

/// A plan to approve: what stays, goes, arrives and cannot be done, and the raw steps behind it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanView {
    pub heading: String,
    /// Which agents the plan reaches, when the user may narrow it.
    pub targets: Option<Targets>,
    pub groups: Vec<(Group, Vec<String>)>,
    pub steps: Vec<String>,
}

/// The agents a plan offers to act on, each checked or not, and the one under the cursor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Targets {
    pub agents: Vec<(Agent, bool)>,
    pub cursor: usize,
}

impl Targets {
    /// All of `agents`, checked.
    pub fn all(agents: Vec<Agent>) -> Self {
        Targets {
            agents: agents.into_iter().map(|agent| (agent, true)).collect(),
            cursor: 0,
        }
    }

    pub fn checked(&self) -> Vec<Agent> {
        self.agents
            .iter()
            .filter(|(_, on)| *on)
            .map(|(agent, _)| *agent)
            .collect()
    }

    pub fn toggled(mut self) -> Self {
        if let Some((_, on)) = self.agents.get_mut(self.cursor) {
            *on = !*on;
        }
        self
    }

    pub fn moved(mut self, down: bool) -> Self {
        let last = self.agents.len().saturating_sub(1);
        self.cursor = match down {
            true => (self.cursor + 1).min(last),
            false => self.cursor.saturating_sub(1),
        };
        self
    }
}

impl PlanView {
    pub fn lines(&self, group: Group) -> &[String] {
        self.groups
            .iter()
            .find(|(g, _)| *g == group)
            .map_or(&[], |(_, lines)| lines.as_slice())
    }

    /// Applying it removes something.
    pub fn destructive(&self) -> bool {
        !self.lines(Group::Remove).is_empty()
    }
}

/// What the user decided about a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Apply,
    /// The user changed the agents; plan again for these.
    Retarget(Targets),
    /// Show that it would run, change nothing.
    DryRun,
    Cancel,
}

/// How a switch ended.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub removed: usize,
    pub installed: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failures: Vec<String>,
    /// Items a removal reported as still installed.
    pub still_installed: Vec<String>,
    /// The step labels that ran, in order.
    pub log: Vec<String>,
    /// How each item ended, in order.
    pub items: Vec<ItemEvent>,
}

impl Outcome {
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty() && self.still_installed.is_empty()
    }

    /// Everything that went wrong, one problem per line, for pasting into a report.
    pub fn problems(&self) -> String {
        let left = self
            .still_installed
            .iter()
            .map(|name| format!("still installed after removal: {name}"));
        self.failures
            .iter()
            .cloned()
            .chain(left)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// What to do after reading an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Done,
    /// Plan and run again: what already landed is recorded, so only the rest is left.
    Retry,
}

/// What a change to one item works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Install(Spec),
    /// The id of the installed item.
    Remove(String),
    /// The id of the installed item.
    Update(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Started,
    Succeeded,
    /// Why it did not work.
    Failed(String),
}

/// One item's change starting or ending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemEvent {
    pub target: Target,
    /// The item's name as the user knows it.
    pub label: String,
    pub phase: Phase,
}

impl ItemEvent {
    pub fn started(target: Target, label: String) -> Self {
        ItemEvent {
            target,
            label,
            phase: Phase::Started,
        }
    }

    /// The same item, now ended with `result`.
    pub fn ended(&self, result: Result<(), String>) -> Self {
        let phase = match result {
            Ok(()) => Phase::Succeeded,
            Err(reason) => Phase::Failed(reason),
        };
        ItemEvent {
            phase,
            ..self.clone()
        }
    }
}

/// Told about each step, and each item, as it starts and ends.
pub trait Progress {
    fn step(&self, label: &str);

    fn item(&self, _event: &ItemEvent) {}
}

/// For prompters with nowhere to show progress.
pub struct NoProgress;

impl Progress for NoProgress {
    fn step(&self, _label: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_is_destructive_only_when_it_removes() {
        let removes = PlanView {
            groups: vec![(Group::Remove, vec!["x".into()])],
            ..PlanView::default()
        };
        let installs = PlanView {
            groups: vec![(Group::Install, vec!["x".into()])],
            ..PlanView::default()
        };

        // Installing alone is not destructive; removing is.
        assert_eq!(
            (removes.destructive(), installs.destructive()),
            (true, false)
        );
    }

    #[test]
    fn problems_list_failures_then_leftovers() {
        let outcome = Outcome {
            failures: vec!["boom".into()],
            still_installed: vec!["a".into(), "b".into()],
            ..Outcome::default()
        };

        let lines = outcome.problems().lines().count();

        // One failure and two leftovers make three lines.
        assert_eq!((lines, outcome.is_clean()), (3, false));
    }
}
