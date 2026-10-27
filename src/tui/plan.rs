//! The plan to approve before a switch: what stays, goes and arrives, answered with Cancel and a
//! button named for the change. The raw steps and a dry run are a key away, unlisted. A plan that
//! removes something is framed in the danger colour.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::app::{Screen, Step};
use super::keymap::{self, Action, Scope};
use super::theme::Theme;
use super::widgets::buttons::Buttons;
use super::widgets::panel::Panel;
use super::widgets::text_modal::{Page, TextModal};
use crate::app::review::{Group, PlanView, Targets, Verdict};

/// Marks the line the cursor is on.
const CURSOR: &str = "▌ ";

/// Where the cancelling and the approving button sit in the row.
const CANCEL: usize = 0;
const APPROVE: usize = 1;

pub struct PlanScreen {
    view: PlanView,
    raw: bool,
    dry_run: bool,
    modal: TextModal,
    buttons: Buttons,
    /// The cursor moved, so the next draw scrolls it into view.
    follow: bool,
}

impl PlanScreen {
    /// Approving is focused, so Enter applies; cancelling is a key or a button away.
    pub fn new(view: PlanView) -> Self {
        let approve = match view.destructive() {
            true => "Delete",
            false => "Apply",
        };
        PlanScreen {
            buttons: Buttons::new(&["Cancel", approve], APPROVE),
            follow: view.targets.is_some(),
            view,
            raw: false,
            dry_run: false,
            modal: TextModal::default(),
        }
    }

    fn text(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.raw {
            return step_lines(&self.view, theme);
        }
        let mut lines = self
            .view
            .targets
            .as_ref()
            .map(|targets| target_lines(targets, theme))
            .unwrap_or_default();
        lines.extend(group_lines(&self.view, theme));
        lines
    }

    fn on_key(&mut self, key: KeyEvent) -> Step<Verdict> {
        match keymap::resolve(Scope::Plan, key) {
            Some(Action::Select) => Step::Act(self.focused()),
            Some(Action::Yes) => Step::Act(self.accepted()),
            Some(Action::Back | Action::No | Action::Quit) => Step::Act(Verdict::Cancel),
            Some(Action::Left) => {
                self.buttons.previous();
                Step::Stay
            }
            Some(Action::Right) => {
                self.buttons.next();
                Step::Stay
            }
            Some(Action::Interrupt) => Step::Quit,
            Some(Action::Toggle) => match self.checkable() {
                Some(targets) => Step::Act(Verdict::Retarget(targets.clone().toggled())),
                None => Step::Stay,
            },
            Some(movement @ (Action::Up | Action::Down)) if self.checkable().is_some() => {
                if let Some(targets) = self.view.targets.take() {
                    self.view.targets = Some(targets.moved(movement == Action::Down));
                }
                self.follow = true;
                Step::Stay
            }
            Some(Action::ToggleDryRun) => self.toggle(|s| &mut s.dry_run),
            Some(Action::ToggleSteps) => self.toggle(|s| &mut s.raw),
            Some(movement) => {
                self.modal.navigate(movement);
                Step::Stay
            }
            None => Step::Stay,
        }
    }

    fn toggle(&mut self, flag: fn(&mut Self) -> &mut bool) -> Step<Verdict> {
        let flag = flag(self);
        *flag = !*flag;
        self.modal.reset();
        self.name_approval();
        Step::Stay
    }

    /// A dry run only previews, and the button says so.
    fn name_approval(&mut self) {
        let label = match (self.dry_run, self.view.destructive()) {
            (true, _) => "Dry run",
            (false, true) => "Delete",
            (false, false) => "Apply",
        };
        self.buttons.set_label(APPROVE, label);
    }

    /// The boxes the user can check, when the plan has any.
    fn checkable(&self) -> Option<&Targets> {
        self.view
            .targets
            .as_ref()
            .filter(|targets| targets.rows() > 0)
    }

    /// What the focused button answers.
    fn focused(&self) -> Verdict {
        match self.buttons.focus() {
            CANCEL => Verdict::Cancel,
            _ => self.accepted(),
        }
    }

    fn accepted(&self) -> Verdict {
        if self.dry_run {
            Verdict::DryRun
        } else {
            Verdict::Apply
        }
    }
}

impl Screen<Verdict> for PlanScreen {
    fn handle(&mut self, event: &Event) -> Step<Verdict> {
        match event {
            Event::Key(key) => self.on_key(*key),
            other => {
                self.modal.wheel(other);
                Step::Stay
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let lines = self.text(theme);
        let reveal = self.follow.then(|| cursor_line(&lines)).flatten();
        self.follow = false;
        let page = Page {
            panel: Panel::new(self.view.heading.clone()).danger(self.view.destructive()),
            lines,
            reveal,
            buttons: Some(self.buttons.line(theme)),
            help: None,
        };
        self.modal.render(frame, theme, page);
    }
}

/// The agents the plan may reach, each checked or not, the cursor on one.
fn target_lines(targets: &Targets, theme: &Theme) -> Vec<Line<'static>> {
    if targets.agents.is_empty() {
        return vec![];
    }
    let glyphs = &theme.glyphs;
    let mut lines = vec![Line::styled(
        "Agents",
        theme.accent.add_modifier(theme.title.add_modifier),
    )];
    lines.extend(targets.agents.iter().enumerate().map(|(at, (agent, on))| {
        let (glyph, style) = match on {
            true => (glyphs.checked, theme.ok),
            false => (glyphs.unchecked, theme.muted),
        };
        let cursor = if at == targets.cursor { CURSOR } else { "  " };
        Line::from(vec![
            Span::styled(cursor, theme.accent),
            Span::styled(format!("{glyph} {agent}"), style),
        ])
    }));
    lines.push(Line::default());
    lines
}

fn group_style(group: Group, theme: &Theme) -> Style {
    match group {
        Group::Keep => theme.muted,
        Group::Remove => theme.danger,
        Group::Install | Group::Update => theme.ok,
        Group::Skip => theme.warn,
    }
}

/// Each non-empty group under a heading with its count, its entries coloured by what happens
/// to them.
fn group_lines(view: &PlanView, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for group in Group::ALL {
        let picks = view
            .targets
            .as_ref()
            .map(|targets| check_lines(targets, group, theme))
            .unwrap_or_default();
        let entries = view.lines(group);
        if picks.is_empty() && entries.is_empty() {
            continue;
        }
        let style = group_style(group, theme);
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let count = if picks.is_empty() {
            entries.len()
        } else {
            picks.len()
        };
        let head = format!("{} ({count})", group.heading());
        lines.push(Line::styled(
            head,
            style.add_modifier(theme.title.add_modifier),
        ));
        if picks.is_empty() {
            lines.extend(entries.iter().cloned().map(|e| Line::styled(e, style)));
        } else {
            lines.extend(picks);
        }
    }
    lines
}

/// The items of `group` the user may leave out, each with a box; the cursor marks one.
fn check_lines(targets: &Targets, group: Group, theme: &Theme) -> Vec<Line<'static>> {
    let glyphs = &theme.glyphs;
    targets
        .items
        .iter()
        .enumerate()
        .filter(|(_, pick)| pick.group == group)
        .map(|(at, pick)| {
            let cursor = if targets.agents.len() + at == targets.cursor {
                CURSOR
            } else {
                "  "
            };
            let (glyph, style) = match pick.on {
                true => (glyphs.checked, group_style(group, theme)),
                false => (glyphs.unchecked, theme.muted),
            };
            Line::from(vec![
                Span::styled(cursor, theme.accent),
                Span::styled(format!("{glyph} {}", pick.label), style),
            ])
        })
        .collect()
}

/// The line the cursor is on.
fn cursor_line(lines: &[Line<'static>]) -> Option<usize> {
    lines
        .iter()
        .position(|line| line.spans.first().is_some_and(|s| s.content == CURSOR))
}

fn step_lines(view: &PlanView, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(
        format!("{} steps, in order", view.steps.len()),
        theme.muted,
    )];
    lines.extend(view.steps.iter().enumerate().map(|(i, step)| {
        Line::from(vec![
            Span::styled(format!("{:>3} ", i + 1), theme.muted),
            Span::raw(step.clone()),
        ])
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::testing::{key, render_to_buffer};
    use ratatui::crossterm::event::KeyCode;
    use skillz_harness::model::Agent;

    fn view() -> PlanView {
        PlanView {
            heading: "t".into(),
            groups: vec![
                (Group::Keep, vec!["a".into()]),
                (Group::Remove, vec!["b".into(), "c".into()]),
            ],
            steps: vec!["run x".into()],
            targets: None,
        }
    }

    fn with_agents() -> PlanScreen {
        PlanScreen::new(PlanView {
            targets: Some(Targets::all(vec![Agent::Claude, Agent::Codex])),
            ..view()
        })
    }

    #[test]
    fn space_unchecks_the_agent_under_the_cursor_and_asks_for_a_new_plan() {
        let mut screen = with_agents();

        answer(&mut screen, KeyCode::Down);
        let verdict = answer(&mut screen, KeyCode::Char(' '));

        let Some(Verdict::Retarget(targets)) = verdict else {
            panic!("expected a new plan, got {verdict:?}");
        };
        assert_eq!(targets.checked(), [Agent::Claude]);
        assert_eq!(targets.cursor, 1);
    }

    #[test]
    fn without_agents_space_does_nothing() {
        let mut screen = PlanScreen::new(view());

        assert_eq!(answer(&mut screen, KeyCode::Char(' ')), None);
    }

    #[test]
    fn the_agents_are_listed_above_the_plan() {
        let theme = Theme::dark();
        let plain = PlanScreen::new(view()).text(&theme).len();

        let listed = with_agents().text(&theme).len();

        // A heading, one line per agent and a blank line.
        assert_eq!(listed, plain + 4);
    }

    fn answer(screen: &mut PlanScreen, code: KeyCode) -> Option<Verdict> {
        match screen.handle(&Event::Key(key(code))) {
            Step::Act(v) => Some(v),
            _ => None,
        }
    }

    fn drawn_text(screen: &mut PlanScreen) -> String {
        let theme = Theme::dark();
        let buffer = render_to_buffer(100, 30, |f| screen.draw(f, &theme));
        (0..30)
            .map(|y| crate::tui::widgets::testing::row_text(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_modal_ends_in_cancel_and_delete_buttons_with_no_help_text() {
        let mut screen = PlanScreen::new(view());

        let text = drawn_text(&mut screen);

        assert!(text.find("[ Cancel ]").unwrap() < text.find("[ Delete ]").unwrap());
        // The old key hints: apply, dry run, steps.
        assert!(!text.contains("dry run") && !text.contains("steps") && !text.contains("enter"));
    }

    fn pick(group: Group, id: &str, on: bool) -> crate::app::review::Pick {
        crate::app::review::Pick {
            group,
            id: id.into(),
            label: id.into(),
            on,
        }
    }

    fn with_items() -> PlanScreen {
        PlanScreen::new(PlanView {
            groups: vec![(Group::Install, vec!["a".into(), "b".into()])],
            targets: Some(Targets {
                agents: vec![],
                items: vec![
                    pick(Group::Install, "a", true),
                    pick(Group::Install, "b", true),
                ],
                cursor: 0,
            }),
            ..view()
        })
    }

    #[test]
    fn each_item_to_install_has_a_box_and_space_unchecks_the_one_under_the_cursor() {
        let mut screen = with_items();

        answer(&mut screen, KeyCode::Down);
        let verdict = answer(&mut screen, KeyCode::Char(' '));

        let Some(Verdict::Retarget(targets)) = verdict else {
            panic!("expected a new plan, got {verdict:?}");
        };
        let on: Vec<bool> = targets.items.iter().map(|p| p.on).collect();
        assert_eq!(on, [true, false]);
        assert_eq!(targets.cursor, 1);
    }

    #[test]
    fn the_items_are_drawn_checked_or_not_under_their_heading() {
        let mut screen = with_items();
        if let Some(targets) = screen.view.targets.as_mut() {
            targets.items[1].on = false;
        }

        let text = drawn_text(&mut screen);

        let theme = Theme::dark();
        assert!(text.contains(&format!("{} a", theme.glyphs.checked)));
        assert!(text.contains(&format!("{} b", theme.glyphs.unchecked)));
    }

    #[test]
    fn a_plan_with_no_boxes_leaves_up_and_down_and_space_to_the_modal() {
        let mut screen = PlanScreen::new(PlanView {
            targets: Some(Targets::default()),
            ..view()
        });

        assert_eq!(answer(&mut screen, KeyCode::Char(' ')), None);
    }

    #[test]
    fn installs_and_removals_are_listed_under_their_own_headings() {
        let mut screen = PlanScreen::new(PlanView {
            groups: vec![
                (Group::Remove, vec!["y".into()]),
                (Group::Install, vec!["x".into(), "z".into()]),
            ],
            ..view()
        });

        let text = drawn_text(&mut screen);

        let (removing, installing) = (
            text.find("removing (1)").unwrap(),
            text.find("installing (2)").unwrap(),
        );
        assert!(removing < installing);
    }

    #[test]
    fn a_plan_that_removes_nothing_is_approved_with_apply() {
        let mut screen = PlanScreen::new(PlanView {
            groups: vec![(Group::Install, vec!["a".into()])],
            ..view()
        });

        let text = drawn_text(&mut screen);

        assert!(text.contains("[ Apply ]") && !text.contains("Delete"));
    }

    #[test]
    fn arrows_or_tab_move_the_focus_and_enter_answers_the_focused_button() {
        let mut screen = PlanScreen::new(view());

        answer(&mut screen, KeyCode::Left);
        let on_cancel = answer(&mut screen, KeyCode::Enter);
        answer(&mut screen, KeyCode::Tab);
        let on_delete = answer(&mut screen, KeyCode::Enter);

        assert_eq!(on_cancel, Some(Verdict::Cancel));
        assert_eq!(on_delete, Some(Verdict::Apply));
    }

    #[test]
    fn y_approves_and_n_cancels_whichever_button_is_focused() {
        let mut screen = PlanScreen::new(view());
        answer(&mut screen, KeyCode::Left);

        let yes = answer(&mut screen, KeyCode::Char('y'));
        let no = answer(&mut screen, KeyCode::Char('n'));

        assert_eq!((yes, no), (Some(Verdict::Apply), Some(Verdict::Cancel)));
    }

    #[test]
    fn a_dry_run_renames_the_approving_button() {
        let mut screen = PlanScreen::new(view());

        answer(&mut screen, KeyCode::Char('d'));

        assert!(drawn_text(&mut screen).contains("[ Dry run ]"));
    }

    #[test]
    fn enter_applies_and_esc_cancels() {
        let verdicts =
            [KeyCode::Enter, KeyCode::Esc].map(|code| answer(&mut PlanScreen::new(view()), code));

        // Apply is the default answer.
        assert_eq!(verdicts, [Some(Verdict::Apply), Some(Verdict::Cancel)]);
    }

    #[test]
    fn with_dry_run_on_enter_only_previews() {
        let mut screen = PlanScreen::new(view());

        answer(&mut screen, KeyCode::Char('d'));
        let verdict = answer(&mut screen, KeyCode::Enter);

        assert_eq!(verdict, Some(Verdict::DryRun));
    }

    #[test]
    fn toggling_steps_switches_between_groups_and_raw_steps() {
        let theme = Theme::dark();
        let mut screen = PlanScreen::new(view());
        let grouped = screen.text(&theme).len();

        answer(&mut screen, KeyCode::Char('s'));
        let raw = screen.text(&theme).len();

        // Grouped: the keep and remove blocks; raw: a heading and the one step.
        assert_eq!((grouped, raw), (2 + 4, 2));
    }

    #[test]
    fn removed_entries_are_drawn_in_the_danger_colour() {
        let theme = Theme::dark();

        let lines = group_lines(&view(), &theme);

        // A removed entry carries the danger style; a kept one does not.
        assert_eq!(lines[4].style, theme.danger);
        assert_eq!(lines[1].style, theme.muted);
    }

    #[test]
    fn a_plan_that_removes_has_a_danger_border() {
        let theme = Theme::dark();
        let mut screen = PlanScreen::new(view());

        let buffer = render_to_buffer(100, 30, |f| screen.draw(f, &theme));

        let corner = buffer.content.iter().find(|c| c.symbol() == "╭");
        // The top-left corner of the modal is drawn in the danger colour.
        assert_eq!(corner.map(|c| c.fg), theme.danger.fg);
    }

    #[test]
    fn control_c_quits_and_control_y_cannot_approve_a_plan() {
        use crate::tui::widgets::testing::ctrl;
        let mut screen = PlanScreen::new(view());

        let modified_yes = screen.handle(&Event::Key(ctrl('y')));
        let interrupt = screen.handle(&Event::Key(ctrl('c')));

        // Assert modified text cannot approve a destructive plan and Ctrl-C exits it.
        assert!(matches!(modified_yes, Step::Stay));
        assert!(matches!(interrupt, Step::Quit));
    }

    #[test]
    fn end_reaches_the_last_visual_line_of_a_wrapped_entry() {
        let mut screen = PlanScreen::new(PlanView {
            heading: String::new(),
            groups: vec![(Group::Keep, vec!["x".repeat(225)])],
            steps: vec![],
            targets: None,
        });
        let theme = Theme::dark();
        render_to_buffer(24, 8, |frame| screen.draw(frame, &theme));

        screen.handle(&Event::Key(key(KeyCode::End)));
        let bottom = render_to_buffer(24, 8, |frame| screen.draw(frame, &theme));
        // The body is 18 cells wide, inside the border and the padding, and ends on row 3.
        let occupied = (3..21)
            .filter(|&x| bottom[(x, 3)].symbol().chars().any(|c| !c.is_whitespace()))
            .count();

        // Assert the last partial visual row is reachable, rather than an earlier full row.
        assert_eq!(occupied, 225 % 18);
    }
}
