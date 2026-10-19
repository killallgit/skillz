use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

/// Indices of `candidates` that match `query`, best match first.
/// Substring matches (case-insensitive) win; only when there are none do fuzzy matches count.
/// An empty query matches everything in original order.
pub fn fuzzy_rank(query: &str, candidates: &[String]) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..candidates.len()).collect();
    }
    let needle = query.to_lowercase();
    let substring: Vec<bool> = candidates
        .iter()
        .map(|c| c.to_lowercase().contains(&needle))
        .collect();
    let any_substring = substring.contains(&true);
    let matcher = SkimMatcherV2::default().ignore_case();
    let mut scored: Vec<(i64, usize)> = candidates
        .iter()
        .enumerate()
        .filter(|(i, _)| !any_substring || substring[*i])
        .filter_map(|(i, c)| matcher.fuzzy_match(c, query).map(|s| (s, i)))
        .collect();
    // Stable on ties so equal scores keep input order.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Does `name` match one comma-free `pattern`? Glob syntax (`*commit`, `speckit-*`) when the
/// pattern has wildcards, exact match otherwise.
pub fn pattern_matches(pattern: &str, name: &str) -> bool {
    match glob::Pattern::new(pattern) {
        Ok(p) if pattern.contains(['*', '?', '[']) => p.matches(name),
        _ => pattern == name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn empty_query_keeps_everything_in_order() {
        assert_eq!(fuzzy_rank("  ", &s(&["b", "a"])), vec![0, 1]);
    }

    #[test]
    fn fuzzy_filters_and_ranks_best_first() {
        let c = s(&["caveman-commit", "tdd", "git-commit"]);
        let r = fuzzy_rank("gitcom", &c);
        assert_eq!(r.first(), Some(&2));
        assert!(!r.contains(&1));
    }

    #[test]
    fn substring_matches_shadow_fuzzy_ones() {
        let c = s(&[
            "knowledge-work",
            "hooks",
            "speckit-plan",
            "setup-pre-commit skills",
        ]);
        assert_eq!(fuzzy_rank("hook", &c), vec![1]);
        assert_eq!(fuzzy_rank("speckit", &c), vec![2]);
        assert_eq!(fuzzy_rank("spkpln", &c), vec![2]);
    }

    #[test]
    fn fuzzy_ignores_case() {
        assert_eq!(fuzzy_rank("TDD", &s(&["tdd"])), vec![0]);
    }

    #[test]
    fn glob_and_exact_patterns() {
        assert!(pattern_matches("*commit", "caveman-commit"));
        assert!(!pattern_matches("*commit", "commit-msg"));
        assert!(pattern_matches("speckit-*", "speckit-plan"));
        assert!(pattern_matches("tdd", "tdd"));
        assert!(!pattern_matches("tdd", "tdd2"));
        assert!(!pattern_matches("[bad", "[bad2"));
    }
}
