//! Fixtures shared by the tests of the `hooks` modules.

use super::HookEntry;

pub(crate) fn hook_entry(event: &str, matcher: Option<&str>, command: &str) -> HookEntry {
    HookEntry {
        event: event.into(),
        matcher: matcher.map(Into::into),
        command: command.into(),
    }
}

pub(crate) const CODEX_TOML: &str = r#"# my config
[features]
codex_hooks = true

[[hooks.SessionStart]]
matcher = "startup|resume"
[[hooks.SessionStart.hooks]]
type = "command"
command = "bd codex-hook SessionStart"
statusMessage = "loading"

[hooks]
Stop = [{ hooks = [{ type = "command", command = "rtk hook codex" }, { type = "command", command = "bd codex-hook Stop" }] }]

[hooks.state."x@y:hooks.json:0"]
trusted = true

[plugins."x@y"]
enabled = true
"#;

pub(crate) const HOOKS_TOML: &str = r#"[features]
x = 1

# start hooks
[[hooks.SessionStart]]
matcher = "startup"
[[hooks.SessionStart.hooks]]
command = "rtk"
[[hooks.SessionStart.hooks]]
command = "keep" # keep me

[[hooks.SessionStart]]
[[hooks.SessionStart.hooks]]
command = "rtk"

[hooks]
Stop = [{ hooks = [{ command = "rtk" }] }, { hooks = [] }, "odd"]

[hooks.state."a@b:x"]
trusted = true

[plugins."a@b"]
enabled = true
"#;
