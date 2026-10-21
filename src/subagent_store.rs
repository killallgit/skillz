//! Where subagent definitions come from: copying them from a source repo or directory, and
//! noting where each came from in `.skillz-sources.json` next to them, which is how a profile
//! snapshot can name the source.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::read_optional::read_optional;
use crate::atomic::write_atomic;
use crate::clone::{check_source, with_tmp_clone};
use crate::fswalk;
use crate::redact::redact_text;
use crate::runner::{Cmd, CommandRunner};

pub(crate) const MANIFEST: &str = ".skillz-sources.json";

/// `.skillz-sources.json`: which source each copied definition file came from.
#[derive(Default)]
pub(crate) struct Manifest(BTreeMap<String, String>);

impl Manifest {
    /// Empty when there is no manifest yet; a manifest that cannot be read or parsed is an
    /// error, not an empty one, so writing back never discards what it recorded.
    pub(crate) fn read(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(MANIFEST);
        match read_optional(&path)? {
            Some(text) => serde_json::from_str(&text)
                .map(Manifest)
                .with_context(|| format!("invalid {}", path.display())),
            None => Ok(Manifest::default()),
        }
    }

    pub(crate) fn get(&self, file: &str) -> Option<&str> {
        self.0.get(file).map(String::as_str)
    }

    fn insert(&mut self, file: &str, source: &str) {
        self.0.insert(file.to_string(), source.to_string());
    }

    fn write(&self, dir: &Path) -> anyhow::Result<()> {
        write_atomic(&dir.join(MANIFEST), &serde_json::to_string_pretty(&self.0)?)
    }
}

/// The agent's own name for a definition: `name:` in markdown frontmatter, `name =` in toml.
pub(crate) fn definition_name(text: &str, ext: &str) -> Option<String> {
    match ext {
        "toml" => text
            .parse::<toml_edit::DocumentMut>()
            .ok()?
            .get("name")?
            .as_str()
            .map(String::from),
        _ => {
            let mut lines = text.lines();
            if lines.next()?.trim_end() != "---" {
                return None;
            }
            lines
                .take_while(|l| l.trim_end() != "---")
                .find_map(|l| l.strip_prefix("name:"))
                .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
                .filter(|n| !n.is_empty())
        }
    }
}

/// `Frontend Developer` and `frontend-developer` are the same name.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// What `copy` needs.
pub struct CopyRequest<'a> {
    pub source: &'a str,
    pub cwd: &'a Path,
    pub names: &'a [String],
    pub ext: &'a str,
    pub to: &'a Path,
}

/// Fetch `source` (a directory relative to `cwd`, else a git repo), copy the definitions called
/// `names` into `to` and record their source. Refuses to overwrite a file skillz did not put
/// there. Returns the files written.
pub fn copy(runner: &dyn CommandRunner, req: &CopyRequest) -> anyhow::Result<Vec<PathBuf>> {
    check_source(req.source)?;
    let local = req.cwd.join(req.source);
    if local.is_dir() {
        return copy_from(&local, req);
    }
    with_tmp_clone(
        req.source,
        |cmd| clone_quietly(runner, req, cmd),
        |tmp| copy_from(tmp, req),
    )
}

fn clone_quietly(
    runner: &dyn CommandRunner,
    req: &CopyRequest,
    (program, args): &Cmd,
) -> anyhow::Result<()> {
    let out = runner.run_quiet(program, args, req.cwd)?;
    anyhow::ensure!(
        out.success,
        "cloning {} failed: {}",
        req.source,
        redact_text(out.stderr.trim())
    );
    Ok(())
}

fn copy_from(root: &Path, req: &CopyRequest) -> anyhow::Result<Vec<PathBuf>> {
    let chosen = pick_definitions(root, req)?;
    fs::create_dir_all(req.to)?;
    let mut manifest = Manifest::read(req.to)?;
    let mut written = vec![];
    for path in &chosen {
        written.push(copy_definition(path, req, &mut manifest)?);
    }
    manifest.write(req.to)?;
    Ok(written)
}

/// The files under `root` for each of `req.names`, matched by file stem or the agent's own name.
fn pick_definitions(root: &Path, req: &CopyRequest) -> anyhow::Result<Vec<PathBuf>> {
    let found = find_definitions(root, req.ext)?;
    req.names
        .iter()
        .map(|wanted| {
            let want = slug(wanted);
            found
                .iter()
                .find(|(path, name)| matches_name(path, name, &want))
                .map(|(path, _)| path.clone())
                .with_context(|| format!("no subagent called {wanted} in {}", req.source))
        })
        .collect()
}

fn matches_name(path: &Path, name: &str, want: &str) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    slug(stem) == want || slug(name) == want
}

fn copy_definition(
    path: &Path,
    req: &CopyRequest,
    manifest: &mut Manifest,
) -> anyhow::Result<PathBuf> {
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("file name")?;
    let dest = req.to.join(file);
    anyhow::ensure!(
        !dest.exists() || manifest.get(file).is_some(),
        "{} exists and was not installed by skillz",
        dest.display()
    );
    fs::copy(path, &dest).with_context(|| format!("copying {}", path.display()))?;
    manifest.insert(file, req.source);
    Ok(dest)
}

/// Every `ext` file under `root` that declares a name, with that name, sorted by path. Files
/// without a name are left out, which keeps READMEs and docs from counting as definitions.
fn find_definitions(root: &Path, ext: &str) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut found = vec![];
    fswalk::walk(root, &mut |path, is_dir| {
        if is_dir || !path.extension().is_some_and(|e| e == ext) {
            return;
        }
        let text = fs::read_to_string(path).unwrap_or_default();
        if let Some(name) = definition_name(&text, ext) {
            found.push((path.to_path_buf(), name));
        }
    })?;
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_frontmatter_or_toml() {
        assert_eq!(
            definition_name("---\nname: \"Frontend Developer\"\n---\n", "md").as_deref(),
            Some("Frontend Developer")
        );
        assert_eq!(definition_name("# readme\nname: x\n", "md"), None);
        assert_eq!(definition_name("---\ndescription: d\n---\n", "md"), None);
        assert_eq!(
            definition_name("name = \"cx\"\n", "toml").as_deref(),
            Some("cx")
        );
        assert_eq!(definition_name("not toml [", "toml"), None);
        assert_eq!(slug("Frontend  Developer!"), "frontend-developer");
    }
}
