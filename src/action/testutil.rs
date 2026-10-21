//! Fixtures shared by the tests of the `action` modules.

use std::fs;
use std::path::Path;

use serde_json::Value;

use super::{Action, Edit, RemovalTarget, apply_edit};
use crate::runner::CommandRunner;

pub(crate) fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

pub(crate) const JSON: &str = "x.json";
pub(crate) const TOML: &str = "x.toml";

pub(crate) fn json_after(edit: &Edit, doc: &Value) -> Value {
    let src = serde_json::to_string_pretty(doc).unwrap();
    serde_json::from_str(&apply_edit(Path::new(JSON), edit, &src).unwrap()).unwrap()
}

pub(crate) fn toml_after(edit: &Edit, src: &str) -> toml_edit::DocumentMut {
    apply_edit(Path::new(TOML), edit, src)
        .unwrap()
        .parse()
        .unwrap()
}

pub(crate) fn hook(event: &str, command: &str) -> Edit {
    Edit::RemoveHook {
        event: event.into(),
        command: command.into(),
    }
}

pub(crate) const CODEX: &str = r#"# top comment
model = "o3"

[marketplaces.mp]
source_type = "git" # inline comment
source = "https://github.com/org/repo.git"

# plugin comment
[plugins."x@mp"]
enabled = true

[plugins."keep@mp"]
enabled = false
"#;

pub(crate) fn write(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

pub(crate) fn rm(path: &Path) -> Action {
    let target = RemovalTarget::new(path.parent().unwrap(), [path.file_name().unwrap()]);
    Action::RemovePath(target.unwrap())
}

pub(crate) struct Recorder {
    pub calls: std::cell::RefCell<Vec<String>>,
    pub fail: bool,
}

impl CommandRunner for Recorder {
    fn run(
        &self,
        program: &str,
        args: &[String],
        _cwd: &Path,
    ) -> anyhow::Result<crate::runner::Output> {
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", args.join(" ")));
        Ok(crate::runner::Output {
            success: !self.fail,
            stderr: "boom".into(),
        })
    }
}

pub(crate) fn run_action(program: &str, arg: &str, cwd: &Path) -> Action {
    Action::Run {
        program: program.into(),
        args: vec![arg.into()],
        cwd: cwd.into(),
        fallback: false,
    }
}
