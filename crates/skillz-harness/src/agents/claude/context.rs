//! Parsing of `claude -p "/context"` output.

/// Rows of the `### Skills` and `### Custom Agents` tables of `/context`: loose skills as
/// `skill:<name>`, and everything from a plugin summed under `plugin:<plugin>`. Built-in and
/// claude.ai skills are not ours to remove and are left out.
pub(super) fn parse_context(text: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut section = "";
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("### ") {
            section = title.trim();
            continue;
        }
        if !matches!(section, "Skills" | "Custom Agents") {
            continue;
        }
        out.extend(context_row(line, section));
    }
    out
}

fn context_row(line: &str, section: &str) -> Option<(String, u32)> {
    let cells: Vec<&str> = line
        .trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    let [name, source, tokens] = cells[..] else {
        return None;
    };
    let tokens = context_tokens(tokens)?;
    Some((context_key(name, source, section)?, tokens))
}

fn context_key(name: &str, source: &str, section: &str) -> Option<String> {
    match (source, section) {
        ("User" | "Project", "Skills") => Some(format!("skill:{name}")),
        ("Plugin", "Custom Agents") => {
            Some(format!("plugin:{}", name.split(':').next().unwrap_or(name)))
        }
        _ => source
            .strip_prefix("Plugin (")
            .and_then(|s| s.strip_suffix(')'))
            .map(|plugin| format!("plugin:{plugin}")),
    }
}

/// `~100`, `1.2k`, `< 20` (counted as 10); header and separator cells are `None`.
fn context_tokens(cell: &str) -> Option<u32> {
    if let Some(small) = cell.strip_prefix('<') {
        return small.trim().parse::<u32>().ok().map(|n| n / 2);
    }
    let cell = cell.trim_start_matches('~');
    match cell.strip_suffix('k') {
        Some(k) => Some((k.parse::<f64>().ok()? * 1000.0).round() as u32),
        None => cell.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_tokens_forms() {
        let t = context_tokens;
        assert_eq!(t("~100"), Some(100));
        assert_eq!(t("1.2k"), Some(1200));
        assert_eq!(t("~1.3k"), Some(1300));
        assert_eq!(t("< 20"), Some(10));
        assert_eq!(t("Tokens"), None);
        assert_eq!(t("------"), None);
    }

    #[test]
    fn context_report_gives_loose_skills_and_per_plugin_sums() {
        let text = include_str!("../../../tests/fixtures/claude_context.md");

        let rows = parse_context(text);

        let sum = |key: &str| -> u32 { rows.iter().filter(|(k, _)| k == key).map(|r| r.1).sum() };
        assert_eq!(sum("skill:tdd"), 50);
        assert_eq!(sum("skill:dataviz"), 0, "built-in skills are not ours");
        // caveman: 21 skills plus 3 agents in the fixture
        assert!(sum("plugin:caveman") > 1500);
        assert!(
            rows.iter()
                .all(|(k, _)| k.starts_with("skill:") || k.starts_with("plugin:"))
        );
    }
}
