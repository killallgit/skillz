//! What a switch shows the user before, while and after it runs, as typed data. Screens decide
//! how to draw it; flows decide how to build it.

use skillz_harness::Spec;
use skillz_harness::model::Agent;

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

    /// How the plan modal titles the group: what is being done to its entries.
    pub fn heading(self) -> &'static str {
        match self {
            Group::Keep => "keeping",
            Group::Remove => "removing",
            Group::Install => "installing",
            Group::Update => "updating",
            Group::Skip => "skipping",
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

/// One item a plan installs or removes, which the user may leave out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub group: Group,
    /// What names it across plans: the entry as text, or the installed item's id.
    pub id: String,
    pub label: String,
    pub on: bool,
}

/// What a plan offers to narrow: the agents it reaches and the items it installs or removes,
/// each checked or not, and the one under the cursor (the agents first, then the items).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Targets {
    /// Empty when the plan reaches no more than one agent: there is nothing to choose.
    pub agents: Vec<(Agent, bool)>,
    pub items: Vec<Pick>,
    pub cursor: usize,
}

impl Targets {
    /// All of `agents`, checked; none are offered for one agent or fewer.
    pub fn all(agents: Vec<Agent>) -> Self {
        let offered = match agents.len() > 1 {
            true => agents,
            false => vec![],
        };
        Targets {
            agents: offered.into_iter().map(|agent| (agent, true)).collect(),
            items: vec![],
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

    /// The agents to plan for, when the user can choose among them.
    pub fn narrowing(&self) -> Option<Vec<Agent>> {
        (!self.agents.is_empty()).then(|| self.checked())
    }

    /// How many lines the cursor can rest on.
    pub fn rows(&self) -> usize {
        self.agents.len() + self.items.len()
    }

    /// Whether the item called `id` is left in the plan; one the plan does not list is.
    pub fn includes(&self, id: &str) -> bool {
        self.items.iter().find(|p| p.id == id).is_none_or(|p| p.on)
    }

    /// The plan's items as they are now, keeping what the user already unchecked.
    pub fn sync_items(&mut self, listed: Vec<(Group, String, String)>) {
        let before = std::mem::take(&mut self.items);
        self.items = listed
            .into_iter()
            .map(|(group, id, label)| Pick {
                on: before.iter().find(|p| p.id == id).is_none_or(|p| p.on),
                group,
                id,
                label,
            })
            .collect();
        self.cursor = self.cursor.min(self.rows().saturating_sub(1));
    }

    pub fn toggled(mut self) -> Self {
        let agents = self.agents.len();
        if let Some((_, on)) = self.agents.get_mut(self.cursor) {
            *on = !*on;
        } else if let Some(pick) = self.items.get_mut(self.cursor - agents) {
            pick.on = !pick.on;
        }
        self
    }

    pub fn moved(mut self, down: bool) -> Self {
        let last = self.rows().saturating_sub(1);
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

    fn listed() -> Vec<(Group, String, String)> {
        vec![
            (Group::Install, "a".into(), "a".into()),
            (Group::Remove, "b".into(), "b".into()),
        ]
    }

    #[test]
    fn syncing_items_keeps_what_was_unchecked_and_includes_new_ones() {
        let mut targets = Targets::all(vec![]);
        targets.sync_items(listed());
        targets.cursor = 1;
        let targets = targets.toggled();

        let mut again = targets.clone();
        again.sync_items(vec![
            (Group::Remove, "b".into(), "b".into()),
            (Group::Install, "c".into(), "c".into()),
        ]);

        assert!(!again.includes("b") && again.includes("c") && again.includes("unlisted"));
        assert!(!targets.includes("b") && targets.includes("a"));
    }

    #[test]
    fn the_cursor_runs_over_the_agents_then_the_items() {
        let mut targets = Targets::all(vec![Agent::Claude, Agent::Codex]);
        targets.sync_items(listed());

        let at_item = targets.clone().moved(true).moved(true);
        let toggled = at_item.clone().toggled();

        assert_eq!(targets.rows(), 4);
        assert_eq!(toggled.checked(), [Agent::Claude, Agent::Codex]);
        assert!(!toggled.includes("a"));
        assert_eq!(toggled.moved(true).moved(true).moved(true).cursor, 3);
    }

    #[test]
    fn one_agent_or_none_offers_no_agent_choice() {
        assert_eq!(Targets::all(vec![Agent::Claude]).narrowing(), None);
        assert_eq!(
            Targets::all(vec![Agent::Claude, Agent::Codex]).narrowing(),
            Some(vec![Agent::Claude, Agent::Codex])
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
