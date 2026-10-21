//! The real `skillz` binary run against a sandbox: HOME, CLAUDE_CONFIG_DIR and CODEX_HOME all
//! point into a temp dir, and `npx` is a script that logs its arguments and succeeds.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use super::read_json;

pub struct Sandbox {
    pub _tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub project: PathBuf,
    pub claude: PathBuf,
    pub codex: PathBuf,
    bin: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let sandbox = Sandbox {
            home: root.join("home"),
            project: root.join("proj"),
            claude: root.join("claude-cfg"),
            codex: root.join("codex-cfg"),
            bin: root.join("bin"),
            _tmp: tmp,
        };
        for dir in [&sandbox.home, &sandbox.project, &sandbox.bin] {
            fs::create_dir_all(dir).unwrap();
        }
        sandbox.fake_program("npx", "echo \"$@\" >> \"$HOME/npx.log\"\n");
        sandbox
    }

    /// An executable on the sandbox's PATH running `body`.
    pub fn fake_program(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.bin.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn skillz_dir(&self) -> PathBuf {
        self.home.join(".config/skillz")
    }

    pub fn write_profile(&self, name: &str, text: &str) {
        let profile = skillz::profile::Profile::parse(name, text).unwrap();
        skillz::profile::Store::new(self.skillz_dir().join("store.json"))
            .save(&profile)
            .unwrap();
    }

    pub fn skillz(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_skillz"))
            .args(args)
            .current_dir(&self.project)
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("CLAUDE_CONFIG_DIR", &self.claude)
            .env("CODEX_HOME", &self.codex)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    }

    /// What `npx` was called with, one line per call.
    pub fn npx_calls(&self) -> Vec<String> {
        fs::read_to_string(self.home.join("npx.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    pub fn project_claude_hooks(&self) -> PathBuf {
        self.project.join(".claude/settings.local.json")
    }

    pub fn project_codex_hooks(&self) -> PathBuf {
        self.project.join(".codex/hooks.json")
    }
}

/// Every command registered under `event` in a nested hooks file (`hooks.<event>[].hooks[]`).
pub fn hook_commands(file: &Path, event: &str) -> Vec<String> {
    if !file.exists() {
        return vec![];
    }
    read_json(file)["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
        .filter_map(|h| h["command"].as_str().map(String::from))
        .collect()
}

/// A profile holding one hook entry for the given agents (a toml list body such as `"claude"`).
pub fn hook_profile(event: &str, command: &str, agents: &str) -> String {
    format!(
        "[[entry]]\nkind = \"hook\"\nevent = \"{event}\"\ncommand = \"{command}\"\nagents = [{agents}]\n"
    )
}
