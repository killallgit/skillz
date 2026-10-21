//! Removal as data: pick the items, turn them into a plan, run it, check what is left.
//! Nothing here prints or prompts; `cli` renders a `Plan` and an `Outcome`.

use anyhow::bail;

use crate::action::{self, Action, Report};
use crate::agents::{self, AgentAdapter};
use crate::env::Env;
use crate::matcher;
use crate::model::{Agent, Augmentation, Scope};
use crate::runner::CommandRunner;

/// Items named exactly `query` (case-insensitive, surrounding whitespace ignored).
pub fn named<'a>(items: &'a [Augmentation], query: &str) -> Vec<&'a Augmentation> {
    items
        .iter()
        .filter(|i| i.name.eq_ignore_ascii_case(query.trim()))
        .collect()
}

/// Items matching `query`: exact (case-insensitive) name matches win over fuzzy ones.
pub fn matches<'a>(items: &'a [Augmentation], query: Option<&str>) -> Vec<&'a Augmentation> {
    let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
        return items.iter().collect();
    };
    let exact = named(items, query);
    if !exact.is_empty() {
        return exact;
    }
    let haystacks: Vec<String> = items
        .iter()
        .map(|i| {
            format!(
                "{} {} {}",
                i.name,
                i.group,
                i.source.as_deref().unwrap_or("")
            )
        })
        .collect();
    matcher::fuzzy_rank(query, &haystacks)
        .into_iter()
        .map(|i| &items[i])
        .collect()
}

/// The items `uninstall QUERY` may offer. `yes` skips the picker and takes every candidate,
/// so it is only allowed when exactly one item has that exact name.
pub fn candidates<'a>(
    items: &'a [Augmentation],
    query: Option<&str>,
    yes: bool,
) -> anyhow::Result<Vec<&'a Augmentation>> {
    let found = matches(items, query);
    if found.is_empty() {
        bail!("nothing installed matches");
    }
    if yes && query.map_or(0, |q| named(items, q).len()) != 1 {
        bail!(
            "{} items match; refine the query or run without --yes to pick",
            found.len()
        );
    }
    Ok(found)
}

/// What removing `chosen` would do.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan<'a> {
    pub chosen: Vec<&'a Augmentation>,
    /// Every step of every chosen item, in order (`action::execute` drops duplicates).
    pub steps: Vec<Action>,
    /// Removal is always complete, so when an agent filter narrowed the view: per item, the
    /// agents outside the filter that lose it too.
    pub also_removes: Vec<(&'a Augmentation, Vec<Agent>)>,
}

impl<'a> Plan<'a> {
    pub fn new(chosen: &[&'a Augmentation], agent_filter: &[Agent]) -> Self {
        let also_removes = chosen
            .iter()
            .filter_map(|&c| {
                let others: Vec<Agent> = c
                    .agents
                    .iter()
                    .copied()
                    .filter(|a| !agent_filter.is_empty() && !agent_filter.contains(a))
                    .collect();
                (!others.is_empty()).then_some((c, others))
            })
            .collect();
        Plan {
            chosen: chosen.to_vec(),
            steps: chosen.iter().flat_map(|c| c.removal.clone()).collect(),
            also_removes,
        }
    }

    /// Remove every one of `items`, with no agent filter.
    pub fn from_owned(items: &'a [Augmentation]) -> Self {
        let refs: Vec<&Augmentation> = items.iter().collect();
        Plan::new(&refs, &[])
    }

    /// The name of the first chosen item whose plan holds `step`.
    pub fn owner(&self, step: &Action) -> &str {
        self.chosen
            .iter()
            .find(|c| c.removal.contains(step))
            .map_or("", |c| c.name.as_str())
    }
}

/// What running a plan did.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub report: Report,
    /// Chosen items the inventory still finds afterwards; empty when the removal worked.
    pub still_installed: Vec<String>,
}

/// What a removal runs against and re-reads the inventory from.
pub struct Removal<'a> {
    pub adapters: &'a [Box<dyn AgentAdapter>],
    pub env: &'a Env,
    pub scopes: &'a [Scope],
    pub runner: &'a dyn CommandRunner,
}

/// Run the plan, then re-read the inventory to see whether every chosen item is gone.
/// `on_step` is told about each step as it starts.
pub fn run(
    plan: &Plan,
    removal: &Removal,
    on_step: &mut dyn FnMut(&Action),
) -> anyhow::Result<Outcome> {
    let report = action::execute_with(&plan.steps, removal.runner, on_step)?;
    let after = agents::inventory(removal.adapters, removal.env, removal.scopes);
    let still_installed = plan
        .chosen
        .iter()
        .filter(|c| after.items.iter().any(|i| i.id == c.id))
        .map(|c| c.name.clone())
        .collect();
    Ok(Outcome {
        report,
        still_installed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Installer, Kind};

    fn item(name: &str, agents: &[Agent], removal: Vec<Action>) -> Augmentation {
        Augmentation {
            id: name.into(),
            name: name.into(),
            kind: Kind::Skill,
            agents: agents.to_vec(),
            scope: Scope::Global,
            group: "g".into(),
            source: None,
            installer: Installer::Local,
            children: vec![],
            removal,
        }
    }

    fn step(name: &str) -> Action {
        Action::native("tool", &["remove", name], &[name], "/".as_ref()).unwrap()
    }

    #[test]
    fn exact_names_win_over_fuzzy_and_yes_needs_exactly_one() {
        let items = [
            item("tdd", &[Agent::Claude], vec![]),
            item("tdd-extra", &[Agent::Claude], vec![]),
        ];
        assert_eq!(matches(&items, Some("TDD")).len(), 1);
        assert_eq!(matches(&items, Some("td")).len(), 2);
        assert_eq!(matches(&items, None).len(), 2);
        assert!(candidates(&items, Some("tdd"), true).is_ok());
        assert!(candidates(&items, Some("td"), true).is_err());
        assert!(candidates(&items, Some("td"), false).is_ok());
        assert!(candidates(&items, Some("zzz"), false).is_err());
    }

    #[test]
    fn plan_collects_steps_and_names_agents_outside_the_filter() {
        let a = item("a", &[Agent::Claude, Agent::Codex], vec![step("a")]);
        let b = item("b", &[Agent::Claude], vec![step("b")]);
        let plan = Plan::new(&[&a, &b], &[Agent::Claude]);
        assert_eq!(plan.steps, [step("a"), step("b")]);
        assert_eq!(plan.also_removes.len(), 1);
        assert_eq!(plan.also_removes[0].1, [Agent::Codex]);
        assert_eq!(plan.owner(&step("b")), "b");
        assert_eq!(plan.owner(&step("zzz")), "");
        assert!(Plan::new(&[&a], &[]).also_removes.is_empty());
    }

    #[test]
    fn run_reports_what_changed_and_reports_nothing_left_with_no_inventory() {
        use crate::runner::NoCommands;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("skills/x");
        std::fs::create_dir_all(&file).unwrap();
        let target = crate::action::RemovalTarget::new(dir.path(), ["skills", "x"]).unwrap();
        let a = item("x", &[Agent::Claude], vec![Action::RemovePath(target)]);
        let plan = Plan::new(&[&a], &[]);
        let env = Env::new(dir.path().into(), dir.path().into());
        let mut seen = 0;
        let removal = Removal {
            adapters: &[],
            env: &env,
            scopes: &[Scope::Global],
            runner: &NoCommands,
        };
        let out = run(&plan, &removal, &mut |_| seen += 1).unwrap();
        assert_eq!(out.report.removed.len(), 1);
        assert!(out.still_installed.is_empty());
        assert_eq!(seen, 1);
        assert!(!file.exists());
    }
}
