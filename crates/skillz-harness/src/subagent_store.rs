//! Where subagent definitions come from: copying them from a source repo or directory, and
//! noting where each came from in `.skillz-sources.json` next to them, which is how a profile
//! snapshot can name the source.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::action::stage;
use crate::clone::with_source;
use crate::fswalk;
use crate::read_optional::read_optional;
use crate::runner::CommandRunner;

pub const MANIFEST: &str = ".skillz-sources.json";

/// `.skillz-sources.json`: which source each copied definition file came from.
#[derive(Default)]
pub struct Manifest(BTreeMap<String, String>);

impl Manifest {
    /// Empty when there is no manifest yet; a manifest that cannot be read or parsed is an
    /// error, not an empty one, so writing back never discards what it recorded.
    pub fn read(dir: &Path) -> anyhow::Result<Self> {
        Self::load(dir).map(|(manifest, _)| manifest)
    }

    fn load(dir: &Path) -> anyhow::Result<(Self, Option<String>)> {
        let path = dir.join(MANIFEST);
        let original = read_optional(&path)?;
        let manifest = match &original {
            Some(text) => serde_json::from_str(text)
                .map(Manifest)
                .with_context(|| format!("invalid {}", path.display())),
            None => Ok(Manifest::default()),
        }?;
        Ok((manifest, original))
    }

    pub fn get(&self, file: &str) -> Option<&str> {
        self.0.get(file).map(String::as_str)
    }

    fn insert(&mut self, file: &str, source: &str) {
        self.0.insert(file.to_string(), source.to_string());
    }
}

/// The agent's own name for a definition: `name:` in markdown frontmatter, `name =` in toml.
pub fn definition_name(text: &str, ext: &str) -> Option<String> {
    match ext {
        "toml" => text
            .parse::<toml_edit::DocumentMut>()
            .ok()?
            .get("name")?
            .as_str()
            .map(String::from),
        _ => crate::agent_skills::parse(text)?.name,
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
    with_source(req.source, req.cwd, runner, |root, source| {
        copy_from(root, source, req)
    })
}

fn copy_from(root: &Path, source: &str, req: &CopyRequest) -> anyhow::Result<Vec<PathBuf>> {
    let chosen = pick_definitions(root, req)?;
    let manifest_path = req.to.join(MANIFEST);
    match fs::symlink_metadata(&manifest_path) {
        Ok(metadata) => anyhow::ensure!(
            !metadata.is_symlink(),
            "refusing to write symlink manifest {}",
            manifest_path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", manifest_path.display()));
        }
    }
    let (mut manifest, original_manifest) = Manifest::load(req.to)?;
    let mut plans = Vec::new();
    let mut destinations = BTreeMap::new();
    for path in chosen {
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("file name")?;
        let dest = req.to.join(file);
        if let Some(previous) = destinations.insert(dest.clone(), path.clone()) {
            anyhow::ensure!(
                previous == path,
                "multiple subagents have destination {}",
                dest.display()
            );
            continue;
        }
        match fs::symlink_metadata(&dest) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_file() && manifest.get(file).is_some(),
                "{} exists and was not installed by skillz",
                dest.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("reading {}", dest.display())),
        }
        let original = read_optional(&dest)?;
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        plans.push((
            dest,
            original,
            text,
            Some(fs::metadata(&path)?.permissions()),
        ));
        manifest.insert(file, source);
    }
    let written = plans.iter().map(|(path, _, _, _)| path.clone()).collect();
    #[cfg(unix)]
    let manifest_permissions = {
        use std::os::unix::fs::PermissionsExt;
        Some(fs::Permissions::from_mode(0o600))
    };
    #[cfg(not(unix))]
    let manifest_permissions = None;
    plans.push((
        manifest_path,
        original_manifest,
        serde_json::to_string_pretty(&manifest.0)?,
        manifest_permissions,
    ));
    let mut staged = Vec::new();
    for (path, original, text, permissions) in plans {
        staged.push(stage::stage_with_permissions(
            &path,
            original,
            &text,
            permissions,
        )?);
    }
    stage::commit(&staged)?;
    Ok(written)
}

/// The files under `root` for each of `req.names`, matched by file stem or the agent's own name.
fn pick_definitions(root: &Path, req: &CopyRequest) -> anyhow::Result<Vec<PathBuf>> {
    let found = find_definitions(root, req.ext)?;
    req.names
        .iter()
        .map(|wanted| {
            let want = slug(wanted);
            let matches: Vec<_> = found
                .iter()
                .filter(|(path, name)| matches_name(path, name, &want))
                .collect();
            anyhow::ensure!(
                matches.len() == 1,
                "expected one subagent called {wanted} in {}, found {}",
                crate::redact::redact_text(req.source),
                matches.len()
            );
            Ok(matches[0].0.clone())
        })
        .collect()
}

fn matches_name(path: &Path, name: &str, want: &str) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    slug(stem) == want || slug(name) == want
}

/// Every `ext` file under `root` that declares a name, with that name, sorted by path. Files
/// without a name are left out, which keeps READMEs and docs from counting as definitions.
fn find_definitions(root: &Path, ext: &str) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut paths = vec![];
    fswalk::walk(root, &mut |path, is_dir| {
        if !is_dir && path.extension().is_some_and(|e| e == ext) {
            paths.push(path.to_path_buf());
        }
    })?;
    let mut found = vec![];
    for path in paths {
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        if let Some(name) = definition_name(&text, ext) {
            found.push((path, name));
        }
    }
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
    fn definition(root: &Path, relative: &str, name: &str) {
        let file = root.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, format!("---\nname: {name}\n---\nbody\n")).unwrap();
    }

    fn copy_names(root: &Path, to: &Path, names: &[&str]) -> anyhow::Result<Vec<PathBuf>> {
        let names = names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        copy(
            &crate::runner::NoCommands,
            &CopyRequest {
                source: root.to_str().unwrap(),
                cwd: root,
                names: &names,
                ext: "md",
                to,
            },
        )
    }

    #[test]
    fn a_later_foreign_destination_leaves_every_file_and_manifest_unchanged() {
        let (source, target) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        definition(source.path(), "a.md", "a");
        definition(source.path(), "b.md", "b");
        fs::write(target.path().join("a.md"), "old managed definition").unwrap();
        fs::write(target.path().join("b.md"), "foreign definition").unwrap();
        fs::write(target.path().join(MANIFEST), r#"{"a.md":"old-source"}"#).unwrap();

        let result = copy_names(source.path(), target.path(), &["a", "b"]);

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(target.path().join("a.md")).unwrap(),
            "old managed definition"
        );
        assert_eq!(
            Manifest::read(target.path()).unwrap().get("a.md"),
            Some("old-source")
        );
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 3);
    }

    #[test]
    fn distinct_definitions_cannot_overwrite_the_same_destination() {
        let (source, target) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        definition(source.path(), "one/shared.md", "first");
        definition(source.path(), "two/shared.md", "second");

        let result = copy_names(source.path(), target.path(), &["first", "second"]);

        assert!(result.is_err());
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
    }

    #[test]
    fn aliases_for_one_definition_are_copied_once_and_ambiguous_names_are_refused() {
        let (source, target) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        definition(source.path(), "helper.md", "Assistant");
        let files = copy_names(source.path(), target.path(), &["helper", "Assistant"]).unwrap();
        assert_eq!(files, [target.path().join("helper.md")]);
        definition(source.path(), "other/helper.md", "Other");
        assert!(copy_names(source.path(), target.path(), &["helper"]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn even_a_recorded_destination_symlink_is_not_written_through() {
        let (source, target) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        definition(source.path(), "helper.md", "helper");
        let victim = target.path().join("victim");
        fs::write(&victim, "keep").unwrap();
        std::os::unix::fs::symlink(&victim, target.path().join("helper.md")).unwrap();
        fs::write(target.path().join(MANIFEST), r#"{"helper.md":"o/r"}"#).unwrap();
        assert!(copy_names(source.path(), target.path(), &["helper"]).is_err());
        assert_eq!(fs::read_to_string(victim).unwrap(), "keep");
    }
    #[cfg(unix)]
    #[test]
    fn copying_preserves_private_source_permissions_and_existing_destination_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (source, target) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        definition(source.path(), "helper.md", "helper");
        let definition = source.path().join("helper.md");
        fs::set_permissions(&definition, fs::Permissions::from_mode(0o600)).unwrap();

        copy_names(source.path(), target.path(), &["helper"]).unwrap();

        let mode = |file: &Path| fs::metadata(file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&target.path().join("helper.md")), 0o600);
        assert_eq!(mode(&target.path().join(MANIFEST)), 0o600);
        fs::set_permissions(definition, fs::Permissions::from_mode(0o644)).unwrap();
        copy_names(source.path(), target.path(), &["helper"]).unwrap();
        assert_eq!(mode(&target.path().join("helper.md")), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_manifests_are_never_written_through_even_when_dangling() {
        let (source, target, outside) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        definition(source.path(), "helper.md", "helper");
        let external = outside.path().join("manifest.json");
        fs::write(&external, "{}").unwrap();
        let manifest = target.path().join(MANIFEST);
        std::os::unix::fs::symlink(&external, &manifest).unwrap();

        assert!(copy_names(source.path(), target.path(), &["helper"]).is_err());
        assert_eq!(fs::read_to_string(&external).unwrap(), "{}");
        assert!(!target.path().join("helper.md").exists());
        fs::remove_file(&external).unwrap();
        assert!(copy_names(source.path(), target.path(), &["helper"]).is_err());
        assert!(!external.exists());
        assert!(fs::symlink_metadata(manifest).unwrap().is_symlink());
    }
}
