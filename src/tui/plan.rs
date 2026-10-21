//! The plan to approve before a switch: what stays, goes and arrives, with the raw steps and a
//! dry run one key away. A plan that removes something is framed in the danger colour.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::app::{Screen, Step};
use super::theme::Theme;
use super::widgets::modal::{self, ModalSize};
use super::widgets::panel::Panel;
use super::widgets::scroll::Scroll;
use crate::review::{Group, PlanView, Verdict};

const WIDTH: u16 = 76;
/// Border, blank line and key hints around the text.
const CHROME: u16 = 4;

pub struct PlanScreen {
    view: PlanView,
    raw: bool,
    dry_run: bool,
    scroll: Scroll,
    /// Lines the last frame drew, which bound the scrolling.
    shown: usize,
}

impl PlanScreen {
    pub fn new(view: PlanView) -> Self {
        PlanScreen {
            view,
            raw: false,
            dry_run: false,
            scroll: Scroll::default(),
            shown: 0,
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
        match key.code {
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => Step::Act(self.accepted()),
            KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => Step::Act(Verdict::Cancel),
            KeyCode::Char('d') => self.toggle(|s| &mut s.dry_run),
            KeyCode::Char('s') => self.toggle(|s| &mut s.raw),
            _ => {
                self.scroll.key(key, self.shown);
                Step::Stay
            }
        }
    }

    fn toggle(&mut self, flag: fn(&mut Self) -> &mut bool) -> Step<Verdict> {
        let flag = flag(self);
        *flag = !*flag;
        self.scroll = Scroll::default();
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
                self.scroll.wheel(other, self.shown);
                Step::Stay
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, theme: &Theme) {
        let text = self.text(theme);
        self.shown = text.len();
        let size = ModalSize {
            width: WIDTH,
            height: (text.len() as u16).saturating_add(CHROME),
        };
        let panel = Panel::new(self.view.heading.clone())
            .focused(true)
            .danger(self.view.destructive());
        let inner = modal::open(frame, size, panel, theme);
        let [body, foot] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        let page = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll.offset(), 0));
        frame.render_widget(page, body);
        frame.render_widget(self.hints(theme), foot);
    }
}

impl PlanScreen {
    fn hints(&self, theme: &Theme) -> Line<'static> {
        let on = |flag: bool, key: &'static str, label: &'static str| {
            let style = if flag { theme.warn } else { theme.muted };
            vec![Span::styled(format!("{key} {label}  "), style)]
        };
        let apply = if self.dry_run { "dry run" } else { "apply" };
        let mut spans = vec![Span::styled(format!("enter {apply}  "), theme.accent)];
        spans.extend(on(self.dry_run, "d", "dry run"));
        spans.extend(on(self.raw, "s", "steps"));
        spans.push(Span::styled("esc cancel", theme.muted));
        Line::from(spans)
    }
}

fn group_style(group: Group, theme: &Theme) -> Style {
    match group {
        Group::Keep => theme.muted,
        Group::Remove => theme.danger,
        Group::Install => theme.ok,
        Group::Skip => theme.warn,
    }
}

fn group_marker(group: Group) -> &'static str {
    match group {
        Group::Keep => "=",
        Group::Remove => "-",
        Group::Install => "+",
        Group::Skip => "~",
    }
}

fn group_name(group: Group) -> &'static str {
    match group {
        Group::Keep => "keep",
        Group::Remove => "remove",
        Group::Install => "install",
        Group::Skip => "skip",
    }
}

/// The counts, then each non-empty group's entries marked and coloured by what happens to them.
fn group_lines(view: &PlanView, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = vec![count_line(view, theme)];
    for group in Group::ALL {
        let entries = view.lines(group);
        if entries.is_empty() {
            continue;
        }
        let style = group_style(group, theme);
        lines.push(Line::default());
        let head = format!("{} ({})", group_name(group), entries.len());
        lines.push(Line::styled(
            head,
            style.add_modifier(theme.title.add_modifier),
        ));
        for entry in entries {
            let marker = format!("  {} ", group_marker(group));
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::raw(entry.clone()),
            ]));
        }
    }
    lines
}

fn count_line(view: &PlanView, theme: &Theme) -> Line<'static> {
    let spans = Group::ALL
        .into_iter()
        .filter(|g| !view.lines(*g).is_empty())
        .map(|g| {
            let text = format!("{}{} ", group_marker(g), view.lines(g).len());
            Span::styled(text, group_style(g, theme))
        });
    Line::from(spans.collect::<Vec<_>>())
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

        // Grouped: counts, then keep and remove blocks; raw: a heading and the one step.
        assert_eq!((grouped, raw), (1 + 3 + 4, 2));
    }

    #[test]
    fn removed_entries_are_drawn_in_the_danger_colour() {
        let theme = Theme::dark();

        let lines = group_lines(&view(), &theme);

        // The remove marker carries the danger style; keep does not.
        assert_eq!(lines[6].spans[0].style, theme.danger);
        assert_eq!(lines[3].spans[0].style, theme.muted);
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
}
