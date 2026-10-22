use std::path::Path;

pub(crate) struct Frontmatter<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub block: &'a str,
}

/// The supported YAML subset: top-level scalar name and scalar or block description.
/// A complete, leading frontmatter block is required for every consumer.
pub(crate) fn parse(text: &str) -> Option<Frontmatter<'_>> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    let start = first.len();
    let (mut name, mut description, mut continued) = (None, None, false);
    let mut offset = start;
    for raw in lines {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line == "---" {
            return Some(Frontmatter {
                name,
                description,
                block: &text[start..offset],
            });
        }
        if continued && line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            description = Some(line.trim());
        }
        if !line.trim().is_empty() && !line.starts_with([' ', '\t']) {
            continued = false;
            match line.split_once(':') {
                Some(("name", value)) => name = scalar(value),
                Some(("description", value)) => {
                    let value = scalar(value);
                    continued = value.is_none() || value.is_some_and(|v| BLOCK.contains(&v));
                    description = if continued { None } else { value };
                }
                _ => {}
            }
        }
        offset += raw.len();
    }
    None
}

const BLOCK: [&str; 6] = [">", "|", ">-", "|-", ">+", "|+"];

fn scalar(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = ['"', '\'']
        .iter()
        .find_map(|quote| value.strip_prefix(*quote)?.strip_suffix(*quote))
        .unwrap_or(value);
    (!value.is_empty()).then_some(value)
}

pub fn skill_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let front = parse(&text)?;
    let name = front.name?;
    front.description?;
    (!name.starts_with('-')).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_subset_accepts_quotes_blocks_and_crlf() {
        for description in ["d", "'d'", "\"d\"", ">\n\n  d", "|-\n  d"] {
            let text = format!("---\nname: 'Name'\ndescription: {description}\n---\nbody");
            let front = parse(&text).unwrap();
            assert_eq!(front.name, Some("Name"));
            assert!(front.description.is_some());
            assert!(!front.block.contains("body"));
        }
        assert!(parse("---\r\nname: a\r\n---\r\n").is_some());
    }

    #[test]
    fn incomplete_or_nonleading_blocks_are_not_frontmatter() {
        assert!(parse("---\nname: x\n").is_none());
        assert!(parse("# intro\n---\nname: x\n---\n").is_none());
        let front = parse("---\nname: ''\ndescription: >\n---\n").unwrap();
        assert!(front.name.is_none() && front.description.is_none());
    }
}
