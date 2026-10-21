//! The workspace home: registered projects on the left, profiles in the middle, and a detail
//! pane for whatever is selected. A pure state machine (`Home::handle`) that only says what the
//! user asked for (`Act`); `cli::workspace` does it and shows the screen again.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{List, ListState, Paragraph};

use super::{event_loop, is_ctrl_c, step_cursor};

pub struct ProjectRow {
    pub path: PathBuf,
    /// Shown in the list: the loaded profile and a short path.
    pub label: String,
    pub missing: bool,
    /// Detail lines: the loaded profile and what it added.
    pub detail: Vec<String>,
    pub loaded: bool,
}

pub struct ProfileRow {
    pub name: String,
    /// Detail lines: one per entry, or why the file is unreadable.
    pub detail: Vec<String>,
    pub broken: bool,
}

#[derive(Default)]
pub struct Model {
    pub projects: Vec<ProjectRow>,
    pub profiles: Vec<ProfileRow>,
    /// Shown in the footer until the next key: what the last action did or why it failed.
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Projects,
    Profiles,
}

/// Where the cursors were, kept across screens so a refresh does not jump back to the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub focus: Focus,
    pub project: usize,
    pub profile: usize,
}

/// What the user asked for. Indices are into the `Model`'s rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Quit,
    Apply { project: usize, profile: usize },
    Unload { project: usize },
    AddProject(String),
    ForgetProject(usize),
    NewProfile(String),
    SaveProfile { name: String, project: usize },
    DeleteProfile(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    AddProject,
    NewProfile,
    SaveProfile,
}

impl Ask {
    fn prompt(self) -> &'static str {
        match self {
            Ask::AddProject => "add project (path)",
            Ask::NewProfile => "new profile (name)",
            Ask::SaveProfile => "save the selected project's setup as profile (name)",
        }
    }
}

pub struct Home {
    model: Model,
    pos: Pos,
    input: Option<(Ask, String)>,
    /// A profile whose deletion waits for `y`.
    deleting: Option<usize>,
}

impl Home {
    pub fn new(model: Model, pos: Pos) -> Self {
        let mut home = Home {
            model,
            pos,
            input: None,
            deleting: None,
        };
        home.pos.project = clamp(home.pos.project, home.model.projects.len());
        home.pos.profile = clamp(home.pos.profile, home.model.profiles.len());
        home
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }

    pub fn handle(&mut self, key: KeyEvent) -> Option<Act> {
        if is_ctrl_c(key) {
            return Some(Act::Quit);
        }
        self.model.notice = None;
        if self.input.is_some() {
            return self.handle_input(key.code);
        }
        if let Some(i) = self.deleting.take() {
            return (key.code == KeyCode::Char('y')).then_some(Act::DeleteProfile(i));
        }
        let (project, profile) = (self.pos.project, self.pos.profile);
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Some(Act::Quit),
            KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
                self.pos.focus = match self.pos.focus {
                    Focus::Projects => Focus::Profiles,
                    Focus::Profiles => Focus::Projects,
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.step(true),
            KeyCode::Down | KeyCode::Char('j') => self.step(false),
            KeyCode::Enter => return self.apply(),
            KeyCode::Char('u') => {
                if self.model.projects.get(project).is_some_and(|p| p.loaded) {
                    return Some(Act::Unload { project });
                }
            }
            KeyCode::Char('a') => self.input = Some((Ask::AddProject, String::new())),
            KeyCode::Char('n') => self.input = Some((Ask::NewProfile, String::new())),
            KeyCode::Char('s') => {
                if self.model.projects.get(project).is_some_and(|p| !p.missing) {
                    self.input = Some((Ask::SaveProfile, String::new()));
                }
            }
            KeyCode::Char('d') => match self.pos.focus {
                Focus::Projects if project < self.model.projects.len() => {
                    return Some(Act::ForgetProject(project));
                }
                Focus::Profiles if profile < self.model.profiles.len() => {
                    self.deleting = Some(profile);
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn apply(&self) -> Option<Act> {
        let project = self.model.projects.get(self.pos.project)?;
        let profile = self.model.profiles.get(self.pos.profile)?;
        (!project.missing && !profile.broken).then_some(Act::Apply {
            project: self.pos.project,
            profile: self.pos.profile,
        })
    }

    fn step(&mut self, up: bool) {
        match self.pos.focus {
            Focus::Projects => {
                self.pos.project = step_cursor(self.pos.project, self.model.projects.len(), up)
            }
            Focus::Profiles => {
                self.pos.profile = step_cursor(self.pos.profile, self.model.profiles.len(), up)
            }
        }
    }

    fn handle_input(&mut self, code: KeyCode) -> Option<Act> {
        let (ask, text) = self.input.as_mut()?;
        match code {
            KeyCode::Char(c) => text.push(c),
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Esc => self.input = None,
            KeyCode::Enter if !text.trim().is_empty() => {
                let (ask, text) = (*ask, text.trim().to_string());
                self.input = None;
                return Some(match ask {
                    Ask::AddProject => Act::AddProject(text),
                    Ask::NewProfile => Act::NewProfile(text),
                    Ask::SaveProfile => Act::SaveProfile {
                        name: text,
                        project: self.pos.project,
                    },
                });
            }
            _ => {}
        }
        None
    }

    fn footer(&self) -> String {
        if let Some((ask, text)) = &self.input {
            return format!("{}: {text}_   (Enter accepts, Esc cancels)", ask.prompt());
        }
        if let Some(i) = self.deleting {
            let name = self.model.profiles.get(i).map_or("", |p| p.name.as_str());
            return format!("delete profile {name}? y to confirm");
        }
        if let Some(notice) = &self.model.notice {
            return notice.clone();
        }
        "Tab pane · Enter apply · u unload · a add project · n new profile · s save as profile · d delete · q quit".into()
    }

    pub fn draw(&self, frame: &mut Frame) {
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new("skillz workspace").style(Style::new().add_modifier(Modifier::BOLD)),
            head,
        );
        let [projects, profiles, detail] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(25),
            Constraint::Percentage(35),
        ])
        .areas(body);
        let focused = self.pos.focus;
        pane(
            frame,
            projects,
            "Projects",
            self.model
                .projects
                .iter()
                .map(|p| {
                    fit_tail(
                        &p.label,
                        (projects.width as usize).saturating_sub(2),
                        PROFILE_COLUMN,
                    )
                })
                .collect(),
            self.pos.project,
            focused == Focus::Projects,
        );
        pane(
            frame,
            profiles,
            "Profiles",
            self.model.profiles.iter().map(|p| p.name.clone()).collect(),
            self.pos.profile,
            focused == Focus::Profiles,
        );
        let lines: Vec<&str> = match focused {
            Focus::Projects => self
                .model
                .projects
                .get(self.pos.project)
                .map(|p| p.detail.iter().map(String::as_str).collect()),
            Focus::Profiles => self
                .model
                .profiles
                .get(self.pos.profile)
                .map(|p| p.detail.iter().map(String::as_str).collect()),
        }
        .unwrap_or_default();
        let [title, text] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(detail);
        frame.render_widget(
            Paragraph::new("Detail").style(Style::new().add_modifier(Modifier::BOLD)),
            title,
        );
        frame.render_widget(Paragraph::new(lines.join("\n")), text);
        frame.render_widget(Paragraph::new(self.footer()), foot);
    }
}

/// The width of the profile column in a project row (`{name:<14} `).
const PROFILE_COLUMN: usize = 15;

/// `label` cut to `width`, keeping its first `head` characters and the end of the rest, since
/// the end of a path says the most.
fn fit_tail(label: &str, width: usize, head: usize) -> String {
    let chars: Vec<char> = label.chars().collect();
    if chars.len() <= width || width <= head + 1 {
        return label.to_string();
    }
    let tail = width - head - 1;
    let start = chars.len() - tail;
    chars[..head]
        .iter()
        .chain(std::iter::once(&'…'))
        .chain(&chars[start..])
        .collect()
}

fn clamp(cursor: usize, len: usize) -> usize {
    cursor.min(len.saturating_sub(1))
}

fn pane(frame: &mut Frame, area: Rect, title: &str, rows: Vec<String>, cursor: usize, focus: bool) {
    let [head, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    let title_style = if focus {
        Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    };
    frame.render_widget(Paragraph::new(title.to_string()).style(title_style), head);
    let shown = if rows.is_empty() { 0 } else { 1 };
    let highlight = if focus {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    };
    let list = List::new(rows).highlight_style(highlight);
    frame.render_stateful_widget(
        list,
        body,
        &mut ListState::default().with_selected((shown > 0).then_some(cursor)),
    );
}

/// Show the home until the user asks for something. Errors when there is no terminal.
pub fn run(model: Model, pos: Pos) -> anyhow::Result<(Act, Pos)> {
    let mut home = Home::new(model, pos);
    let act = event_loop(&mut home, |h, f| h.draw(f), |h, k| h.handle(k))?;
    Ok((act, home.pos()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{key, keys};

    fn project(label: &str, loaded: bool, missing: bool) -> ProjectRow {
        ProjectRow {
            path: label.into(),
            label: label.into(),
            missing,
            detail: vec![],
            loaded,
        }
    }

    fn profile(name: &str, broken: bool) -> ProfileRow {
        ProfileRow {
            name: name.into(),
            detail: vec![],
            broken,
        }
    }

    fn home() -> Home {
        Home::new(
            Model {
                projects: vec![project("a", false, false), project("b", true, false)],
                profiles: vec![profile("x", false), profile("y", false)],
                notice: None,
            },
            Pos::default(),
        )
    }

    fn press(h: &mut Home, s: &str) -> Option<Act> {
        keys(s).into_iter().filter_map(|k| h.handle(k)).last()
    }

    #[test]
    fn tab_moves_between_panes_and_arrows_stay_inside_the_list() {
        let mut h = home();
        press(&mut h, "jjj");
        assert_eq!(h.pos().project, 1);
        h.handle(key(KeyCode::Tab));
        assert_eq!(h.pos().focus, Focus::Profiles);
        press(&mut h, "jjk");
        assert_eq!(h.pos().profile, 0);
        h.handle(key(KeyCode::Left));
        assert_eq!(h.pos().focus, Focus::Projects);
    }

    #[test]
    fn enter_applies_the_selected_profile_to_the_selected_project() {
        let mut h = home();
        press(&mut h, "j");
        h.handle(key(KeyCode::Tab));
        press(&mut h, "j");
        assert_eq!(
            h.handle(key(KeyCode::Enter)),
            Some(Act::Apply {
                project: 1,
                profile: 1
            })
        );
    }

    #[test]
    fn nothing_is_applied_to_a_missing_project_or_with_a_broken_profile() {
        let mut h = home();
        h.model.projects[0].missing = true;
        assert_eq!(h.handle(key(KeyCode::Enter)), None);
        h.model.projects[0].missing = false;
        h.model.profiles[0].broken = true;
        assert_eq!(h.handle(key(KeyCode::Enter)), None);
        let mut empty = Home::new(Model::default(), Pos::default());
        assert_eq!(empty.handle(key(KeyCode::Enter)), None);
        assert_eq!(press(&mut empty, "jjkdus"), None);
    }

    #[test]
    fn unload_needs_a_loaded_profile() {
        let mut h = home();
        assert_eq!(press(&mut h, "u"), None);
        press(&mut h, "j");
        assert_eq!(press(&mut h, "u"), Some(Act::Unload { project: 1 }));
    }

    #[test]
    fn typing_a_name_asks_for_the_matching_action() {
        let mut h = home();
        press(&mut h, "a");
        assert_eq!(press(&mut h, "/tmp/x"), None);
        assert_eq!(
            h.handle(key(KeyCode::Enter)),
            Some(Act::AddProject("/tmp/x".into()))
        );
        press(&mut h, "n");
        press(&mut h, "web");
        h.handle(key(KeyCode::Backspace));
        assert_eq!(
            h.handle(key(KeyCode::Enter)),
            Some(Act::NewProfile("we".into()))
        );
        press(&mut h, "j");
        press(&mut h, "s");
        press(&mut h, "snap");
        assert_eq!(
            h.handle(key(KeyCode::Enter)),
            Some(Act::SaveProfile {
                name: "snap".into(),
                project: 1
            })
        );
    }

    #[test]
    fn input_can_be_cancelled_and_empty_input_is_not_submitted() {
        let mut h = home();
        press(&mut h, "n");
        assert_eq!(h.handle(key(KeyCode::Enter)), None);
        press(&mut h, "q");
        h.handle(key(KeyCode::Esc));
        assert_eq!(press(&mut h, "q"), Some(Act::Quit), "back to normal keys");
    }

    #[test]
    fn deleting_a_profile_waits_for_y_but_forgetting_a_project_does_not() {
        let mut h = home();
        assert_eq!(press(&mut h, "d"), Some(Act::ForgetProject(0)));
        h.handle(key(KeyCode::Tab));
        assert_eq!(press(&mut h, "dn"), None);
        assert_eq!(press(&mut h, "dy"), Some(Act::DeleteProfile(0)));
    }

    #[test]
    fn quitting_and_cursors_surviving_a_refresh() {
        let mut h = home();
        assert_eq!(h.handle(key(KeyCode::Esc)), Some(Act::Quit));
        let ctrl_c = KeyEvent::new(
            KeyCode::Char('c'),
            ratatui::crossterm::event::KeyModifiers::CONTROL,
        );
        assert_eq!(h.handle(ctrl_c), Some(Act::Quit));
        let pos = Pos {
            focus: Focus::Profiles,
            project: 9,
            profile: 1,
        };
        let h = Home::new(Model::default(), pos);
        assert_eq!((h.pos().project, h.pos().profile), (0, 0));
        assert_eq!(h.pos().focus, Focus::Profiles);
    }

    #[test]
    fn long_labels_keep_their_profile_column_and_the_end_of_the_path() {
        let label = "web            ~/Code/a/very/long/project/path";
        assert_eq!(fit_tail(label, 80, 15), label);
        let cut = fit_tail(label, 30, 15);
        assert_eq!(cut.chars().count(), 30);
        assert!(cut.starts_with("web            ") && cut.ends_with("path") && cut.contains('…'));
        assert_eq!(fit_tail(label, 10, 15), label, "too narrow to cut sensibly");
    }

    #[test]
    fn the_footer_says_what_is_being_asked() {
        let mut h = home();
        let idle = h.footer();
        press(&mut h, "n");
        assert_ne!(h.footer(), idle);
        h.handle(key(KeyCode::Esc));
        h.handle(key(KeyCode::Tab));
        press(&mut h, "d");
        assert!(h.footer().contains('x'));
        h.deleting = None;
        h.model.notice = Some("boom".into());
        assert_eq!(h.footer(), "boom");
    }

    #[test]
    fn drawing_does_not_panic_for_empty_and_full_models() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        for model in [
            Model::default(),
            Model {
                projects: vec![project("a", true, false)],
                profiles: vec![profile("x", false)],
                notice: None,
            },
        ] {
            let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
            let h = Home::new(model, Pos::default());
            term.draw(|f| h.draw(f)).unwrap();
        }
    }
}
