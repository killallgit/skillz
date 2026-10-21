//! The workspace screen: a scripted `Home` prompter plays actions and records every screen shown.

mod common;

use std::cell::RefCell;
use std::collections::VecDeque;

use common::*;
use skillz::cli::workspace;
use skillz::profile::Entry;
use skillz::prompt::Prompter;
use skillz::review::{PlanView, Verdict};
use skillz::tui::home::{Act, Model, Pos};

/// What one workspace screen showed: project count, profile names, whether a notice was up.
struct Screen {
    projects: usize,
    profiles: Vec<String>,
    has_notice: bool,
    has_undo: bool,
}

/// Plays a script of workspace actions (then quits) and remembers what each screen showed.
struct Home {
    acts: RefCell<VecDeque<Act>>,
    picks: RefCell<VecDeque<Vec<usize>>>,
    seen: RefCell<Vec<Screen>>,
    verdict: Verdict,
    plans: RefCell<Vec<PlanView>>,
}

impl Home {
    fn playing(acts: Vec<Act>) -> Self {
        Home::picking(acts, vec![])
    }

    fn picking(acts: Vec<Act>, picks: Vec<Vec<usize>>) -> Self {
        Home {
            acts: RefCell::new(acts.into()),
            picks: RefCell::new(picks.into()),
            seen: RefCell::default(),
            verdict: Verdict::Apply,
            plans: RefCell::default(),
        }
    }
}

impl Prompter for Home {
    fn multi_select(&self, _: &str, _: &[String]) -> anyhow::Result<Vec<usize>> {
        Ok(self.picks.borrow_mut().pop_front().unwrap_or_default())
    }
    fn confirm(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn plan(&self, view: &PlanView) -> anyhow::Result<Verdict> {
        self.plans.borrow_mut().push(view.clone());
        Ok(self.verdict)
    }
    fn home(&self, model: Model, pos: Pos) -> anyhow::Result<(Act, Pos)> {
        self.seen.borrow_mut().push(Screen {
            projects: model.projects.len(),
            profiles: model.profiles.iter().map(|p| p.name.clone()).collect(),
            has_notice: model.notice.is_some(),
            has_undo: model.undo.is_some(),
        });
        Ok((self.acts.borrow_mut().pop_front().unwrap_or(Act::Quit), pos))
    }
}

fn world_with_web() -> World {
    let w = world();
    w.write_skill_profile("web", "tdd");
    w
}

/// A project with the skill `tdd` installed from `o/r`, as the project's own skills lock sees it.
fn world_with_project_skill() -> World {
    let w = world();
    write(
        &w.project.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\ndescription: d\n---\n",
    );
    write(
        &w.project.join("skills-lock.json"),
        &serde_json::json!({"skills": {"tdd": {"source": "o/r"}}}).to_string(),
    );
    w
}

fn play(w: &World, home: &Home) {
    let r = Recorder::default();
    let d = deps(w, &r, home);
    workspace(&d).unwrap();
}

fn screens_after(w: &World, acts: Vec<Act>) -> Vec<Screen> {
    let home = Home::playing(acts);
    play(w, &home);
    home.seen.into_inner()
}

fn proj(w: &World) -> std::path::PathBuf {
    w.project.canonicalize().unwrap()
}

fn plugin_spec(profile: &str) -> Act {
    Act::AddTyped {
        profile: profile.into(),
        spec: "plugin o/mp mp pp".into(),
    }
}

// --- projects and profiles

#[test]
fn the_workspace_shows_one_screen_per_action_plus_the_last_quit() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![Act::AddProject(".".into()), Act::NewProfile("tmp".into())],
    );

    assert_eq!(seen.len(), 3);
}

#[test]
fn a_project_added_in_the_workspace_is_shown() {
    let w = world_with_web();

    let seen = screens_after(&w, vec![Act::AddProject(".".into())]);

    assert_eq!(seen[0].projects, 0);
    assert_eq!(seen[1].projects, 1);
}

#[test]
fn a_project_forgotten_in_the_workspace_disappears() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![Act::AddProject(".".into()), Act::ForgetProject(proj(&w))],
    );

    assert_eq!(seen[2].projects, 0);
}

#[test]
fn a_profile_created_in_the_workspace_is_listed_with_the_others() {
    let w = world_with_web();

    let seen = screens_after(&w, vec![Act::NewProfile("tmp".into())]);

    assert_eq!(seen[1].profiles, ["tmp", "web"]);
}

#[test]
fn creating_a_profile_twice_is_reported_not_fatal() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![Act::NewProfile("tmp".into()), Act::NewProfile("tmp".into())],
    );

    assert!(seen[2].has_notice);
    assert_eq!(seen[2].profiles, ["tmp", "web"]);
}

#[test]
fn a_profile_deleted_in_the_workspace_disappears() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![
            Act::NewProfile("tmp".into()),
            Act::DeleteProfile("tmp".into()),
        ],
    );

    assert_eq!(seen[2].profiles, ["web"]);
}

#[test]
fn applying_a_profile_records_it_for_the_project() {
    let w = world_with_web();

    screens_after(
        &w,
        vec![
            Act::AddProject(".".into()),
            Act::Apply {
                project: proj(&w),
                profile: "web".into(),
            },
        ],
    );

    assert_eq!(w.applied_profiles(), ["web"]);
}

#[test]
fn unloading_forgets_the_projects_profile() {
    let w = world_with_web();

    screens_after(
        &w,
        vec![
            Act::AddProject(".".into()),
            Act::Apply {
                project: proj(&w),
                profile: "web".into(),
            },
            Act::Unload { project: proj(&w) },
        ],
    );

    assert!(w.applied_profiles().is_empty());
}

#[test]
fn saving_the_projects_setup_adds_a_profile() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![
            Act::AddProject(".".into()),
            Act::SaveProfile {
                name: "snap".into(),
                project: proj(&w),
            },
        ],
    );

    assert_eq!(seen[2].profiles, ["snap", "web"]);
}

// --- editing a profile

#[test]
fn installed_items_can_be_added_to_a_profile() {
    let w = world_with_project_skill();
    let home = Home::picking(
        vec![
            Act::AddProject(".".into()),
            Act::NewProfile("p".into()),
            Act::AddInstalled {
                profile: "p".into(),
                project: proj(&w),
            },
        ],
        vec![vec![0]],
    );

    play(&w, &home);

    assert!(matches!(
        w.profile("p").entries.as_slice(),
        [Entry::Skill { source, names, .. }] if source == "o/r" && names == &["tdd"]
    ));
}

#[test]
fn a_typed_line_adds_an_entry() {
    let w = world_with_web();

    screens_after(&w, vec![Act::NewProfile("p".into()), plugin_spec("p")]);

    assert!(matches!(
        w.profile("p").entries.as_slice(),
        [Entry::Plugin { source, marketplace, name, .. }]
            if source == "o/mp" && marketplace == "mp" && name == "pp"
    ));
}

#[test]
fn a_bad_typed_line_is_reported_not_fatal() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![
            Act::NewProfile("p".into()),
            Act::AddTyped {
                profile: "p".into(),
                spec: "nonsense".into(),
            },
        ],
    );

    assert!(seen[2].has_notice);
    assert_eq!(seen.len(), 3);
}

#[test]
fn a_bad_typed_line_adds_nothing() {
    let w = world_with_web();

    screens_after(
        &w,
        vec![
            Act::NewProfile("p".into()),
            Act::AddTyped {
                profile: "p".into(),
                spec: "nonsense".into(),
            },
        ],
    );

    assert!(w.profile("p").entries.is_empty());
}

#[test]
fn picked_entries_are_removed_from_a_profile() {
    let w = world_with_project_skill();
    let home = Home::picking(
        vec![
            Act::AddProject(".".into()),
            Act::NewProfile("p".into()),
            Act::AddInstalled {
                profile: "p".into(),
                project: proj(&w),
            },
            plugin_spec("p"),
            Act::RemoveEntries {
                profile: "p".into(),
            },
        ],
        vec![vec![0], vec![0]],
    );

    play(&w, &home);

    assert!(matches!(
        w.profile("p").entries.as_slice(),
        [Entry::Plugin { .. }]
    ));
}

// --- stale ids

#[test]
fn forgetting_a_project_that_is_not_registered_is_reported() {
    let w = world_with_web();

    let seen = screens_after(&w, vec![Act::ForgetProject(proj(&w))]);

    assert!(seen[1].has_notice);
}

#[test]
fn deleting_a_profile_that_is_gone_is_reported_and_touches_nothing() {
    let w = world_with_web();

    let seen = screens_after(&w, vec![Act::DeleteProfile("ghost".into())]);

    assert!(seen[1].has_notice);
    assert_eq!(seen[1].profiles, ["web"]);
}

#[test]
fn applying_to_a_project_that_is_not_registered_is_reported_and_applies_nothing() {
    let w = world_with_web();

    let seen = screens_after(
        &w,
        vec![Act::Apply {
            project: proj(&w),
            profile: "web".into(),
        }],
    );

    assert!(seen[1].has_notice);
    assert!(w.applied_profiles().is_empty());
}

fn apply_web(w: &World) -> Vec<Act> {
    vec![
        Act::AddProject(".".into()),
        Act::Apply {
            project: proj(w),
            profile: "web".into(),
        },
    ]
}

#[test]
fn an_applied_switch_offers_to_undo_itself() {
    let w = world_with_web();

    let seen = screens_after(&w, apply_web(&w));

    // The screen after the apply carries the undo request.
    assert!(seen[2].has_undo);
}

#[test]
fn a_dry_run_shows_the_plan_and_changes_nothing() {
    let w = world_with_web();
    let mut home = Home::playing(apply_web(&w));
    home.verdict = Verdict::DryRun;

    play(&w, &home);

    // The plan was shown once, the ledger is untouched and no undo is offered.
    assert_eq!(home.plans.borrow().len(), 1);
    assert!(w.applied_profiles().is_empty());
    assert!(!home.seen.borrow()[2].has_undo);
}

#[test]
fn a_cancelled_plan_changes_nothing() {
    let w = world_with_web();
    let mut home = Home::playing(apply_web(&w));
    home.verdict = Verdict::Cancel;

    play(&w, &home);

    assert!(w.applied_profiles().is_empty());
}
