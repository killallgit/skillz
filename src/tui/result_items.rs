//! The result screen's list of what was done, item by item.

use ratatui::text::{Line, Span};

use super::theme::Theme;
use crate::review::{ItemEvent, Outcome, Phase, Target};

/// One line per item that worked, in the order they ran. Failures are listed with their
/// reasons by the problems block.
pub(super) fn done_lines(outcome: &Outcome, theme: &Theme) -> Vec<Line<'static>> {
    let done: Vec<&ItemEvent> = outcome
        .items
        .iter()
        .filter(|event| event.phase == Phase::Succeeded)
        .collect();
    if done.is_empty() {
        return vec![];
    }
    let mark = format!("{} ", theme.glyphs.loaded);
    let lines = done.into_iter().map(|event| {
        Line::from(vec![
            Span::styled(mark.clone(), theme.ok),
            Span::raw(format!("{} {}", verb(&event.target), event.label)),
        ])
    });
    std::iter::once(Line::default()).chain(lines).collect()
}

fn verb(target: &Target) -> &'static str {
    match target {
        Target::Install(_) => "Installed",
        Target::Remove(_) => "Removed",
        Target::Update(_) => "Updated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(target: Target, phase: Phase) -> ItemEvent {
        ItemEvent {
            target,
            label: "x".into(),
            phase,
        }
    }

    #[test]
    fn only_items_that_worked_get_a_line_each() {
        let outcome = Outcome {
            items: vec![
                event(Target::Remove("a".into()), Phase::Succeeded),
                event(Target::Update("b".into()), Phase::Failed("boom".into())),
                event(Target::Update("c".into()), Phase::Succeeded),
            ],
            ..Outcome::default()
        };

        let lines = done_lines(&outcome, &Theme::dark());

        // A blank separator, then the two that worked.
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn nothing_done_adds_nothing_to_the_screen() {
        assert!(done_lines(&Outcome::default(), &Theme::dark()).is_empty());
    }
}
