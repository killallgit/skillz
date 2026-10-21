use std::path::Path;

/// The name of the skill in `dir`, unless npx would parse it as a flag.
pub fn skill_name(dir: &Path) -> Option<String> {
    let md = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let name = frontmatter_name(&md)?;
    (!name.starts_with('-')).then(|| name.to_string())
}

/// `name:` from the leading `---` frontmatter block; `None` (npx skips the skill) unless both
/// `name` and `description` are set.
/// A `description:` with no inline value (`>`, `|`, or nothing) counts once an indented line
/// follows it, as in YAML.
// shortcut: top-level keys only, no full YAML; add a parser if real skills still go missing.
fn frontmatter_name(md: &str) -> Option<&str> {
    let mut lines = md.lines();
    if lines.next()? != "---" {
        return None;
    }
    let (mut name, mut description, mut continued) = ("", "", false);
    for line in lines {
        if line == "---" {
            return (!name.is_empty() && !description.is_empty()).then_some(name);
        }
        if continued && line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            description = line.trim();
        }
        continued = false;
        match line.split_once(':') {
            Some(("name", v)) => name = unquote(v),
            Some(("description", v)) => {
                description = unquote(v);
                if description.is_empty() || BLOCK.contains(&description) {
                    (description, continued) = ("", true);
                }
            }
            _ => {}
        }
    }
    None
}

const BLOCK: [&str; 6] = [">", "|", ">-", "|-", ">+", "|+"];

fn unquote(v: &str) -> &str {
    let v = v.trim();
    ['"', '\'']
        .iter()
        .find_map(|q| v.strip_prefix(*q)?.strip_suffix(*q))
        .unwrap_or(v)
}
