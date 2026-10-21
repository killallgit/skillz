use std::fs;
use std::path::Path;

use crate::action::{Action, Edit, RemovalTarget};
use crate::env::Env;
use crate::model::Augmentation;

pub(super) const CONFIG: &str = r#"
model = "gpt-5"

[desktop]
theme = "dark"

[features]
web_search = true

[marketplaces.openai-bundled]
source_type = "local"
source = "/Applications/Codex.app/plugins"

[marketplaces.killall]
source_type = "git"
source = "https://github.com/killallgit/killall-skills.git"

[marketplaces.localdev]
source_type = "local"
source = "/src/my-plugins"

[plugins."browser@openai-bundled"]
enabled = true

[plugins."review@killall"]
enabled = true

[plugins."commit@killall"]
enabled = false

[hooks.state."review@killall:hooks/hooks.json:session_start:0:0"]
trusted_hash = "abc"
enabled = true

[hooks.state."review@killall:hooks/hooks.json:stop:0:0"]
enabled = false

[hooks.state."review@killall-old:hooks/hooks.json:stop:0:0"]
enabled = true
"#;

pub(super) const REVIEW_START: &str = "review@killall:hooks/hooks.json:session_start:0:0";
pub(super) const REVIEW_STOP: &str = "review@killall:hooks/hooks.json:stop:0:0";

/// A removal plan without its native-tool commands.
pub(super) fn files(plan: &[Action]) -> Vec<Action> {
    plan.iter()
        .filter(|a| !matches!(a, Action::Run { .. }))
        .cloned()
        .collect()
}

pub(super) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub(super) fn setup() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let env = Env {
        home: dir.path().join("home"),
        project: dir.path().join("proj"),
        claude_config: None,
        codex_home: None,
    };
    write(&env.home.join(".codex/config.toml"), CONFIG);
    let cache = env.home.join(".codex/plugins/cache/killall");
    write(&cache.join("review/1.0.0/skills/lint/SKILL.md"), "x");
    write(&cache.join("review/1.0.0/skills/diff/SKILL.md"), "x");
    write(&cache.join("review/1.1.0/skills/lint/SKILL.md"), "x");
    write(
        &cache.join("review/1.1.0/hooks/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"say done"}]}]}}"#,
    );
    write(
        &cache.join("commit/0.1.0/hooks.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"say done"}]}]}}"#,
    );
    (dir, env)
}

pub(super) fn find<'a>(items: &'a [Augmentation], id: &str) -> &'a Augmentation {
    items.iter().find(|a| a.id == id).unwrap()
}

pub(super) fn rm_state(file: &Path, key: &str) -> Action {
    Action::Edit {
        file: file.to_path_buf(),
        edit: Edit::TomlRemoveKey {
            path: vec!["hooks".into(), "state".into(), key.into()],
        },
    }
}

pub(super) fn rm(root: &Path, segments: &[&str]) -> Action {
    Action::RemovePath(RemovalTarget::new(root, segments).unwrap())
}

pub(super) fn rm_key(file: &Path, table: &str, key: &str) -> Action {
    Action::Edit {
        file: file.to_path_buf(),
        edit: Edit::TomlRemoveKey {
            path: vec![table.into(), key.into()],
        },
    }
}
