//! A horizontal rule with a label in the middle: `──────── global ────────`. The list's divider
//! and the command line's group headings are both drawn with it.

/// `label` between two runs of `rule`, `width` cells in all and the label centred. A label too
/// wide for that stands alone.
pub fn centered_rule(label: &str, width: usize, rule: &str) -> String {
    let label = format!(" {label} ");
    let room = width.saturating_sub(label.chars().count());
    let (left, right) = (room / 2, room - room / 2);
    format!("{}{label}{}", rule.repeat(left), rule.repeat(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_sits_in_the_middle_and_the_whole_is_exactly_as_wide() {
        let line = centered_rule("global", 20, "─");

        let before = line.chars().take_while(|&c| c == '─').count();
        let after = line.chars().rev().take_while(|&c| c == '─').count();
        assert_eq!(line.chars().count(), 20);
        assert!(before.abs_diff(after) <= 1);
        assert!(line.contains(" global "));
    }

    #[test]
    fn a_label_wider_than_the_width_is_kept_whole() {
        assert_eq!(centered_rule("a-long-name", 4, "─"), " a-long-name ");
    }
}
