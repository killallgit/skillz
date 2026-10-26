//! The dialog that installs one library item: where (this project or the user), and for which
//! of the available coding tools.

use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::tui::keymap::{self, Action};
use crate::tui::theme::Theme;
use crate::tui::widgets::modal::{self, ModalSize};
use crate::tui::widgets::panel::Panel;
use skillz_harness::Spec;
use skillz_harness::model::{Agent, Scope};

const WIDTH: u16 = 44;
/// The scope rows come first; the tool rows follow them.
const SCOPES: [(Scope, &str); 2] = [(Scope::Project, "This project"), (Scope::Global, "Global")];

/// What a key did to the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Verdict {
    Open,
    Cancel,
    Install { scope: Scope, agents: Vec<Agent> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InstallDialog {
    title: String,
    scope: Scope,
    tools: Vec<(Agent, bool)>,
    cursor: usize,
}

impl InstallDialog {
    /// A dialog for `entry` offering `available` tools; the entry's own tools start checked,
    /// or every tool when it names none.
    pub(super) fn new(label: &str, entry: &Spec, available: &[Agent]) -> Self {
        let wanted: Vec<Agent> = entry
            .agents()
            .iter()
            .copied()
            .filter(|agent| available.contains(agent))
            .collect();
        let checked = |agent: &Agent| wanted.is_empty() || wanted.contains(agent);
        InstallDialog {
            title: format!("Install {label}"),
            scope: Scope::Project,
            tools: available.iter().map(|a| (*a, checked(a))).collect(),
            cursor: 0,
        }
    }

    fn rows(&self) -> usize {
        SCOPES.len() + self.tools.len()
    }

    pub(super) fn handle(&mut self, key: KeyEvent) -> Verdict {
        match keymap::resolve(keymap::Scope::Install, key) {
            Some(Action::Up) => self.cursor = self.cursor.saturating_sub(1),
            Some(Action::Down) => self.cursor = (self.cursor + 1).min(self.rows() - 1),
            Some(Action::Toggle) => self.choose(),
            Some(Action::Select) => {
                if let Some(verdict) = self.install() {
                    return verdict;
                }
            }
            Some(Action::Back | Action::Quit | Action::Interrupt) => return Verdict::Cancel,
            _ => {}
        }
        Verdict::Open
    }

    /// The scope under the cursor becomes the scope; a tool under it is switched.
    fn choose(&mut self) {
        match self.cursor.checked_sub(SCOPES.len()) {
            None => self.scope = SCOPES[self.cursor].0,
            Some(tool) => self.tools[tool].1 = !self.tools[tool].1,
        }
    }

    /// Nothing is installed for no tool at all.
    fn install(&self) -> Option<Verdict> {
        let agents: Vec<Agent> = self
            .tools
            .iter()
            .filter(|(_, on)| *on)
            .map(|(agent, _)| *agent)
            .collect();
        (!agents.is_empty()).then_some(Verdict::Install {
            scope: self.scope,
            agents,
        })
    }

    pub(super) fn render(&self, frame: &mut Frame, theme: &Theme) {
        let size = ModalSize {
            width: WIDTH,
            height: (self.rows() + 6) as u16,
        };
        let inner = modal::open(frame, size, Panel::new(self.title.clone()), theme);
        let mut lines = vec![Line::styled("Where", theme.muted)];
        for (at, (scope, label)) in SCOPES.iter().enumerate() {
            let mark = if *scope == self.scope { "(*)" } else { "( )" };
            lines.push(self.row(at, format!("{mark} {label}"), theme));
        }
        lines.push(Line::default());
        lines.push(Line::styled("For", theme.muted));
        for (at, (agent, on)) in self.tools.iter().enumerate() {
            let mark = if *on { "[x]" } else { "[ ]" };
            lines.push(self.row(SCOPES.len() + at, format!("{mark} {agent}"), theme));
        }
        lines.push(Line::default());
        lines.push(Line::styled(
            "space choose · enter install · esc cancel",
            theme.muted,
        ));
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn row(&self, at: usize, text: String, theme: &Theme) -> Line<'static> {
        let style = match at == self.cursor {
            true => theme.text.add_modifier(Modifier::REVERSED | Modifier::BOLD),
            false => theme.text,
        };
        Line::styled(format!(" {text}"), style)
    }
}
