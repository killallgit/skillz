/// Indices of `candidates` that fuzzy-match `query`, best match first.
/// An empty query matches everything in original order.
pub fn fuzzy_rank(query: &str, candidates: &[String]) -> Vec<usize> {
    let _ = (query, candidates);
    todo!("lane: skills")
}

/// Does `name` match one comma-free `pattern`? Glob syntax (`*commit`, `speckit-*`) when the
/// pattern has wildcards, exact match otherwise.
pub fn pattern_matches(pattern: &str, name: &str) -> bool {
    let _ = (pattern, name);
    todo!("lane: skills")
}
