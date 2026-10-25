use std::collections::BTreeMap;
use std::path::Path;

use yaml_rust2::{Yaml, YamlLoader};

use super::SKILL_FILE;

/// The frontmatter of a `SKILL.md` or an agent definition. The spec's own keys are fields; any
/// other key a harness adds (`argument-hint`, `hooks`, ...) is kept in `extra`.
#[derive(Debug, Clone, PartialEq)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: BTreeMap<String, String>,
    /// Space-separated pre-approved tools (experimental in the spec).
    pub allowed_tools: Option<String>,
    pub extra: BTreeMap<String, Yaml>,
    /// The text between the `---` lines, as written.
    pub raw: String,
}

const SPEC_KEYS: [&str; 6] = [
    "name",
    "description",
    "license",
    "compatibility",
    "metadata",
    "allowed-tools",
];

/// The leading `---` block of `text`. Files whose block is not valid YAML (an unquoted colon in a
/// description is common) still give their name and description, read line by line.
/// A complete, leading block is required for every consumer.
pub fn parse(text: &str) -> Option<Frontmatter> {
    let raw = block(text)?;
    let plain = lenient(raw);
    if !safe_for_yaml(raw) {
        return Some(plain);
    }
    Some(match as_map(raw) {
        Some(map) => from_map(&map, raw, &plain),
        None => plain,
    })
}

/// The name of the skill in `dir`, when `SKILL.md` has both a name and a description.
pub fn skill_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(SKILL_FILE)).ok()?;
    let front = parse(&text)?;
    front.description?;
    front.name.filter(|name| !name.starts_with('-'))
}

/// Blocks come from repositories nobody vetted. The YAML parser expands aliases by copying, so
/// a block with anchors or aliases (a "billion laughs") could take unbounded time and memory,
/// and a huge block could nest deeply: those are read by the line reader instead.
fn safe_for_yaml(raw: &str) -> bool {
    const LIMIT: usize = 64 * 1024;
    if raw.len() > LIMIT {
        return false;
    }
    let mut before = ' ';
    for (i, c) in raw.char_indices() {
        let starts_token = before.is_whitespace() || matches!(before, '[' | '{' | ',');
        let next = raw[i + c.len_utf8()..].chars().next();
        if starts_token && matches!(c, '&' | '*') && next.is_some_and(|n| !n.is_whitespace()) {
            return false;
        }
        before = c;
    }
    true
}

fn block(text: &str) -> Option<&str> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    let start = first.len();
    let mut offset = start;
    for raw in lines {
        if raw.trim_end_matches(['\r', '\n']) == "---" {
            return Some(&text[start..offset]);
        }
        offset += raw.len();
    }
    None
}

fn as_map(raw: &str) -> Option<BTreeMap<String, Yaml>> {
    let documents = YamlLoader::load_from_str(raw).ok()?;
    let hash = documents.into_iter().next()?.into_hash()?;
    Some(
        hash.into_iter()
            .filter_map(|(key, value)| Some((text(&key)?, value)))
            .collect(),
    )
}

/// A scalar as the text it was written as; empty means absent.
fn text(value: &Yaml) -> Option<String> {
    let text = match value {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        _ => return None,
    };
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// `name` and `description` are the text as written, also when YAML would read them as a number,
/// a null or a list: those come from the line reader, which keeps the literal text.
fn from_map(map: &BTreeMap<String, Yaml>, raw: &str, plain: &Frontmatter) -> Frontmatter {
    let field = |key: &str| map.get(key).and_then(text);
    let written = |key: &str, line_read: &Option<String>| match map.get(key) {
        Some(Yaml::String(_) | Yaml::Real(_) | Yaml::Boolean(_)) => field(key),
        Some(_) => line_read.clone(),
        None => None,
    };
    let metadata = map
        .get("metadata")
        .and_then(Yaml::as_hash)
        .map(|hash| {
            hash.iter()
                .filter_map(|(k, v)| Some((text(k)?, text(v)?)))
                .collect()
        })
        .unwrap_or_default();
    Frontmatter {
        name: written("name", &plain.name),
        description: written("description", &plain.description),
        license: field("license"),
        compatibility: field("compatibility"),
        metadata,
        allowed_tools: field("allowed-tools"),
        extra: map
            .iter()
            .filter(|(key, _)| !SPEC_KEYS.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        raw: raw.to_owned(),
    }
}

const BLOCK: [&str; 6] = [">", "|", ">-", "|-", ">+", "|+"];

/// Top-level scalar `name` and scalar or block `description`, nothing else.
fn lenient(raw: &str) -> Frontmatter {
    let (mut name, mut description, mut continued) = (None, None, false);
    for line in raw.lines() {
        if continued && line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            description = Some(line.trim().to_owned());
        }
        if !line.trim().is_empty() && !line.starts_with([' ', '\t']) {
            continued = false;
            match line.split_once(':') {
                Some(("name", value)) => name = scalar(value),
                Some(("description", value)) => {
                    let value = scalar(value);
                    continued = value.as_deref().is_none_or(|v| BLOCK.contains(&v));
                    description = if continued { None } else { value };
                }
                _ => {}
            }
        }
    }
    Frontmatter {
        name,
        description,
        license: None,
        compatibility: None,
        metadata: BTreeMap::new(),
        allowed_tools: None,
        extra: BTreeMap::new(),
        raw: raw.to_owned(),
    }
}

fn scalar(value: &str) -> Option<String> {
    let value = value.trim();
    let value = ['"', '\'']
        .iter()
        .find_map(|quote| value.strip_prefix(*quote)?.strip_suffix(*quote))
        .unwrap_or(value);
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_quotes_blocks_and_crlf_give_the_name_and_description() {
        for description in ["d", "'d'", "\"d\"", ">\n\n  d", "|-\n  d"] {
            let text = format!("---\nname: 'Name'\ndescription: {description}\n---\nbody");
            let front = parse(&text).unwrap();
            assert_eq!(front.name.as_deref(), Some("Name"));
            assert!(front.description.is_some());
            assert!(!front.raw.contains("body"));
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

    #[test]
    fn spec_keys_are_fields_and_every_other_key_is_kept() {
        let text = "---\nname: pdf\ndescription: d\nlicense: MIT\ncompatibility: needs git\n\
                    metadata:\n  author: me\n  version: 2\nallowed-tools: Bash(git:*) Read\n\
                    argument-hint: '[file]'\nhooks:\n  PreToolUse: []\n---\n";

        let front = parse(text).unwrap();

        assert_eq!(front.license.as_deref(), Some("MIT"));
        assert_eq!(front.compatibility.as_deref(), Some("needs git"));
        assert_eq!(front.allowed_tools.as_deref(), Some("Bash(git:*) Read"));
        assert_eq!(
            front.metadata.into_iter().collect::<Vec<_>>(),
            [
                ("author".into(), "me".into()),
                ("version".into(), "2".into())
            ]
        );
        assert_eq!(
            front.extra.keys().map(String::as_str).collect::<Vec<_>>(),
            ["argument-hint", "hooks"]
        );
    }

    #[test]
    fn invalid_yaml_still_gives_the_name_and_description() {
        let front = parse("---\nname: x\ndescription: use when: asked\n---\n").unwrap();

        assert_eq!(front.name.as_deref(), Some("x"));
        assert_eq!(front.description.as_deref(), Some("use when: asked"));
        assert!(front.extra.is_empty());
    }

    #[test]
    fn a_name_or_description_yaml_reads_as_a_number_or_null_keeps_its_written_text() {
        let front = parse("---\nname: 007\ndescription: ~\n---\n").unwrap();

        assert_eq!(front.name.as_deref(), Some("007"));
        assert_eq!(front.description.as_deref(), Some("~"));
    }

    #[test]
    fn aliases_and_oversized_blocks_are_read_by_line_not_expanded() {
        let bomb = "---\nname: x\ndescription: d\na: &a [x, x, x, x, x, x, x, x, x]\n\
                    b: &b [*a, *a, *a, *a, *a, *a, *a, *a, *a]\nc: &c [*b, *b, *b, *b, *b, *b, *b, *b, *b]\n---\n";
        let huge = format!(
            "---\nname: x\ndescription: d\nbig: {}\n---\n",
            "y".repeat(70_000)
        );

        for text in [bomb.to_string(), huge] {
            let front = parse(&text).unwrap();
            assert_eq!(front.name.as_deref(), Some("x"));
            assert!(front.extra.is_empty());
        }
    }

    #[test]
    fn an_ampersand_inside_a_word_is_not_an_anchor() {
        let front = parse("---\nname: x\ndescription: R&D and a*b\nlicense: MIT\n---\n").unwrap();

        assert_eq!(front.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn skill_name_needs_a_description_and_no_leading_hyphen() {
        let dir = tempfile::tempdir().unwrap();
        let write = |md: &str| std::fs::write(dir.path().join("SKILL.md"), md).unwrap();

        write("---\nname: ok\ndescription: d\n---\n");
        let ok = skill_name(dir.path());
        write("---\nname: ok\n---\n");
        let bare = skill_name(dir.path());
        write("---\nname: -x\ndescription: d\n---\n");
        let hyphen = skill_name(dir.path());

        assert_eq!((ok.as_deref(), bare, hyphen), (Some("ok"), None, None));
    }
}
