//! The installers an install runs, as the user changes them on the plan: each agent's own
//! plugin installer, and `npx skills`.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::app::prompt::{Choice, Picked};
use crate::app::prompt::{Hold, Prompter};
use crate::app::review::{ItemEvent, Next, Outcome, PlanView, Progress, Targets, Verdict};
use crate::app::tree::Node;
use skillz_harness::model::Agent;

/// Which installers the plan is made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Tools {
    pub agents: Vec<Agent>,
    /// `npx skills` takes every skill the user asked for; checked on the plan.
    pub npx: bool,
    /// An agent whose source offers no plugin gets the source's skills through `npx skills`,
    /// as it always has, until the user has taken `npx` off the plan.
    pub fallback: bool,
}

impl Tools {
    /// What the user's choice on the plan makes of the tools, given whether the plan showed
    /// `npx` checked only because of the fallback.
    pub fn after(&self, chosen: &Targets, shown_by_fallback: bool) -> Tools {
        let wants_npx = chosen.npx_on();
        Tools {
            agents: chosen.checked(),
            // Leaving a fallback box checked keeps the fallback: it picks skills, `npx` takes all.
            npx: wants_npx && !shown_by_fallback,
            fallback: self.fallback && !(shown_by_fallback && !wants_npx),
        }
    }
}

/// Remembers the answers to the questions asked while planning, so changing the installers on
/// the plan does not ask the same question again.
pub(super) struct Remembering<'a> {
    asks: &'a dyn Prompter,
    picked: RefCell<HashMap<String, Vec<usize>>>,
    confirmed: RefCell<HashMap<String, bool>>,
}

impl<'a> Remembering<'a> {
    pub fn new(asks: &'a dyn Prompter) -> Self {
        Remembering {
            asks,
            picked: RefCell::default(),
            confirmed: RefCell::default(),
        }
    }
}

impl Prompter for Remembering<'_> {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        let key = format!("{title}\u{0}{}", options.join("\u{0}"));
        if let Some(picked) = self.picked.borrow().get(&key) {
            return Ok(picked.clone());
        }
        let picked = self.asks.multi_select(title, options)?;
        self.picked.borrow_mut().insert(key, picked.clone());
        Ok(picked)
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
        if let Some(answer) = self.confirmed.borrow().get(question) {
            return Ok(*answer);
        }
        let answer = self.asks.confirm(question)?;
        self.confirmed
            .borrow_mut()
            .insert(question.to_string(), answer);
        Ok(answer)
    }

    fn select_items(&self, title: &str, nodes: &[Node]) -> anyhow::Result<Vec<usize>> {
        self.asks.select_items(title, nodes)
    }

    fn browse(&self, title: &str, groups: &[Node]) -> anyhow::Result<Option<(Choice, Picked)>> {
        self.asks.browse(title, groups)
    }

    fn hold(&self) -> Hold<'_> {
        self.asks.hold()
    }

    fn plan(&self, view: &PlanView) -> anyhow::Result<Verdict> {
        self.asks.plan(view)
    }

    fn progress(&self, title: &str, total: usize) -> Box<dyn Progress + '_> {
        self.asks.progress(title, total)
    }

    fn live(&self, title: &str, total: usize, planned: &[ItemEvent]) -> Box<dyn Progress + '_> {
        self.asks.live(title, total, planned)
    }

    fn outcome(&self, title: &str, outcome: &Outcome) -> anyhow::Result<Next> {
        self.asks.outcome(title, outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::install::test_support::ScriptedPrompter;

    const BOTH: [Agent; 2] = [Agent::Claude, Agent::Codex];

    fn tools(npx: bool, fallback: bool) -> Tools {
        Tools {
            agents: BOTH.to_vec(),
            npx,
            fallback,
        }
    }

    fn chosen(agents: &[Agent], npx: bool) -> Targets {
        Targets::installers(BOTH.to_vec(), agents, npx)
    }

    #[test]
    fn checking_npx_makes_it_take_every_skill() {
        let next = tools(false, true).after(&chosen(&BOTH, true), false);

        assert_eq!(next, tools(true, true));
    }

    #[test]
    fn a_fallback_box_left_checked_keeps_picking_skills_and_unchecking_it_stops_the_fallback() {
        let kept = tools(false, true).after(&chosen(&[Agent::Claude], true), true);
        let dropped = tools(false, true).after(&chosen(&BOTH, false), true);

        assert_eq!(
            kept,
            Tools {
                agents: vec![Agent::Claude],
                npx: false,
                fallback: true
            }
        );
        assert_eq!(dropped, tools(false, false));
    }

    #[test]
    fn unchecking_npx_that_was_checked_takes_it_off() {
        let next = tools(true, true).after(&chosen(&BOTH, false), false);

        assert_eq!(next, tools(false, true));
    }

    #[test]
    fn a_question_asked_again_gets_the_answer_given_before() {
        let asks = ScriptedPrompter::new(&[1], true);
        let remembering = Remembering::new(&asks);
        let options = vec!["a".to_string(), "b".to_string()];

        let first = remembering.multi_select("pick", &options).unwrap();
        let again = remembering.multi_select("pick", &options).unwrap();
        remembering.confirm("sure?").unwrap();
        remembering.confirm("sure?").unwrap();

        assert_eq!((first, again), (vec![1], vec![1]));
        assert_eq!(asks.offered.borrow().len(), 1);
    }
}
