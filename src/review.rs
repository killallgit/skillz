//! What a switch shows the user before, while and after it runs, as typed data. Screens decide
//! how to draw it; flows decide how to build it.

/// The kinds of line in a plan, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Keep,
    Remove,
    Install,
    Skip,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Keep, Group::Remove, Group::Install, Group::Skip];
}

/// A plan to approve: what stays, goes, arrives and cannot be done, and the raw steps behind it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanView {
    pub heading: String,
    pub groups: Vec<(Group, Vec<String>)>,
    pub steps: Vec<String>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Apply,
    /// Show that it would run, change nothing.
    DryRun,
    Cancel,
}

/// How a switch ended.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub removed: usize,
    pub installed: usize,
    pub skipped: usize,
    pub failures: Vec<String>,
    /// Items a removal reported as still installed.
    pub still_installed: Vec<String>,
    /// The step labels that ran, in order.
    pub log: Vec<String>,
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

/// Told about each step as it starts.
pub trait Progress {
    fn step(&self, label: &str);
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
