//! A prompter for plain commands: plans and results are written as lines, and the question
//! about a plan is a yes/no from whoever prompts for the command.

use std::cell::RefCell;
use std::io::Write;

use anyhow::bail;

use super::Notice;
use crate::app::prompt::Prompter;
use crate::app::review::{Group, PlanView, Verdict};

pub(in crate::cli) struct Console<'a> {
    asks: &'a dyn Prompter,
    out: RefCell<&'a mut dyn Write>,
    dry_run: bool,
}

impl<'a> Console<'a> {
    pub fn new(asks: &'a dyn Prompter, out: &'a mut dyn Write, dry_run: bool) -> Self {
        Console {
            asks,
            out: RefCell::new(out),
            dry_run,
        }
    }

    /// Print what a session did; a failure is the command's error, a cancelled plan says so.
    pub fn conclude(&self, notice: Option<Notice>) -> anyhow::Result<()> {
        match notice {
            Some(Notice::Failed(problem)) => bail!(problem),
            Some(done) => self.say(&done.to_string()),
            None => self.say("aborted"),
        }
    }

    pub fn say(&self, line: &str) -> anyhow::Result<()> {
        Ok(writeln!(self.out.borrow_mut(), "{line}")?)
    }
}

impl Prompter for Console<'_> {
    fn multi_select(&self, title: &str, options: &[String]) -> anyhow::Result<Vec<usize>> {
        self.asks.multi_select(title, options)
    }

    fn confirm(&self, question: &str) -> anyhow::Result<bool> {
        self.asks.confirm(question)
    }

    fn plan(&self, view: &PlanView) -> anyhow::Result<Verdict> {
        self.say(&render(view))?;
        if self.dry_run {
            return Ok(Verdict::DryRun);
        }
        Ok(match self.asks.confirm("Proceed?")? {
            true => Verdict::Apply,
            false => Verdict::Cancel,
        })
    }
}

fn render(view: &PlanView) -> String {
    let mut lines = vec![view.heading.clone()];
    for group in Group::ALL {
        let verb = group.name();
        lines.extend(view.lines(group).iter().map(|l| format!("  {verb:<8}{l}")));
    }
    lines.push("Plan:".into());
    lines.extend(view.steps.iter().map(|step| format!("  {step}")));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::prompt::AssumeYes;

    fn view() -> PlanView {
        PlanView {
            heading: "project: new".into(),
            groups: vec![(Group::Install, vec!["skills tdd [o/r]".into()])],
            steps: vec!["npx skills add o/r".into()],
            targets: None,
        }
    }

    #[test]
    fn a_dry_run_plan_is_shown_and_never_applied() {
        let mut out = Vec::new();
        let console = Console::new(&AssumeYes, &mut out, true);

        let verdict = console.plan(&view()).unwrap();

        assert_eq!(verdict, Verdict::DryRun);
        assert!(!out.is_empty());
    }

    #[test]
    fn a_plan_is_applied_when_the_prompter_agrees() {
        let mut out = Vec::new();
        let console = Console::new(&AssumeYes, &mut out, false);

        assert_eq!(console.plan(&view()).unwrap(), Verdict::Apply);
    }

    #[test]
    fn a_failure_is_the_commands_error_and_a_cancelled_plan_is_not() {
        let mut out = Vec::new();
        let console = Console::new(&AssumeYes, &mut out, false);

        let failed = console.conclude(Some(Notice::Failed("broke".into())));
        let cancelled = console.conclude(None);

        assert!(failed.is_err());
        assert!(cancelled.is_ok());
    }
}
