//! The plan to approve before a switch: what stays, goes and arrives, with the raw steps and a
//! dry run one key away. A plan that removes something is framed in the danger colour.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyEvent};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::app::{Screen, Step};
use super::keymap::{self, Action, Scope};
use super::theme::Theme;
use super::widgets::panel::Panel;
use super::widgets::text_modal::{Page, TextModal};
use crate::review::{Group, PlanView, Verdict};

pub struct PlanScreen {
    view: PlanView,
    raw: bool,
    dry_run: bool,
    modal: TextModal,
}

impl PlanScreen {
    pub fn new(view: PlanView) -> Self {
        PlanScreen {
            view,
            raw: false,
            dry_run: false,
            modal: TextModal::default(),
        }
    }

    fn text(&self, theme: &Theme) -> Vec<Line<'static>> {
        if self.raw {
            step_lines(&self.view, theme)
        } else {
            group_lines(&self.view, theme)
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Step<Verdict> {
        match keymap::resolve(Scope::Plan, key) {
            Some(Action::Select) => Step::Act(self.accepted()),
            Some(Action::Back | Action::Quit) => Step::Act(Verdict::Cancel),
            Some(Action::Interrupt) => Step::Quit,
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
        Step::Stay
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
        let page = Page {
            panel: Panel::new(self.view.heading.clone()).danger(self.view.destructive()),
            lines: self.text(theme),
            footer: self.hints(theme),
        };
        self.modal.render(frame, theme, page);
    }
}

impl PlanScreen {
    fn hints(&self, theme: &Theme) -> Line<'static> {
        let spans = keymap::hint_bindings(&[Scope::Plan])
            .into_iter()
            .map(|binding| {
                let active = match binding.action {
                    Action::ToggleDryRun => self.dry_run,
                    Action::ToggleSteps => self.raw,
                    _ => false,
                };
                let label = if binding.action == Action::Select && self.dry_run {
                    "dry run"
                } else {
                    binding.label
                };
                let style = if active { theme.warn } else { theme.muted };
                Span::styled(format!("{} {label}  ", binding.keys), style)
            });
        Line::from(spans.collect::<Vec<_>>())
    }
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
        let entries = view.lines(group);
        if entries.is_empty() {
            continue;
        }
        let style = group_style(group, theme);
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let head = format!("{} ({})", group.name(), entries.len());
        lines.push(Line::styled(
            head,
            style.add_modifier(theme.title.add_modifier),
        ));
        lines.extend(entries.iter().cloned().map(|e| Line::styled(e, style)));
    }
    lines
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

    fn view() -> PlanView {
        PlanView {
            heading: "t".into(),
            groups: vec![
                (Group::Keep, vec!["a".into()]),
                (Group::Remove, vec!["b".into(), "c".into()]),
            ],
            steps: vec!["run x".into()],
        }
    }

    fn answer(screen: &mut PlanScreen, code: KeyCode) -> Option<Verdict> {
        match screen.handle(&Event::Key(key(code))) {
            Step::Act(v) => Some(v),
            _ => None,
        }
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
