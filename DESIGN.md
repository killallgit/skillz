# skillz design

Rust CLI (clap) that lists, installs and completely removes skills, plugins, marketplaces and
hooks for Claude Code, Codex and Cursor, globally (`~`) or per project.

## Principles

- **Discovery is read-only and plans removal up front.** Each discovered `Augmentation`
  carries `removal: Vec<Action>`. Uninstall = concatenate the selected plans, execute.
- **Effects at the edges.** `action::apply_edit` is a pure `&str -> String`. Only
  `action::execute`, `runner::SystemRunner` and `tui::*::run` touch the outside world.
- **Dependency injection where effects are.** `Env` (home/project roots), `CommandRunner`
  (external programs), `Prompter` (user interaction) are passed in. Tests use temp dirs,
  fake runners and scripted prompters. The filesystem is used directly against temp dirs.
- **Open for extension.** New agent = new `agents/<name>.rs` implementing `AgentAdapter` +
  one line in `agents::registry()`. New augmentation storage = new `Edit` variant.
- **Tests never assert on user-facing strings** (messages, error text, rendered labels).
  Assert on structure: ids, kinds, counts, `Action`s, parsed file contents, tree shape.
- Coverage target > 75% (`cargo llvm-cov`).

## Module map

| Module | Owns |
| --- | --- |
| `model` | `Agent`, `Kind`, `Scope`, `Augmentation` |
| `env` | `Env { home, project }` |
| `action` | `Action`, `Edit`, `apply_edit` (pure), `execute` (effects: dedupe, plan, all-or-nothing commit, path removal) |
| `hooks` | every hook layout, both ways: `Format::of(file)`, `parse` and `remove_hook` (pure, one walker per format), `hook_group`, `discover(file)` |
| `skills` | shared loose-skill discovery (npx `skills` aware) |
| `npx_skills` | the npx `skills` format: store dir and lock path per scope, lock parsing (name -> source), agent ids, `add` command line |
| `matcher` | `fuzzy_rank`, `pattern_matches` (glob) |
| `agents/*` | per-agent plugin/marketplace discovery + `skill_dirs` + `hook_files` |
| `tree` | display tree build + text render |
| `tui/*` | multi-select list, collapsible tree, `/` filter |
| `prompt` | `Prompter` trait, `TuiPrompter`, `AssumeYes` |
| `runner` | `CommandRunner` trait, `SystemRunner` |
| `install` | install flow: `install()` dispatch, `InstallRequest`, `SkillSelection` |
| `install::source` | source fetch: reject empty/option-like sources, local path or `git clone --depth 1` into a temp dir, credential-redacted `run_checked` |
| `install::skills` | npx `skills` installer: `scan_available`, selection/patterns, runs `npx_skills::add_command` |
| `install::marketplace` | Claude marketplace installer: reads `.claude-plugin/marketplace.json`, runs `claude plugin marketplace add` + `plugin install` |
| `cli` | clap definitions, wiring, list/uninstall/install commands |

## On-disk formats (observed on a real machine)

### Claude Code
- Settings: `~/.claude/settings.json` (global), `<proj>/.claude/settings.json`,
  `<proj>/.claude/settings.local.json`. Keys: `hooks`, `enabledPlugins` (`{"name@mp": bool}`),
  `extraKnownMarketplaces` (`{"mp": {...}}`).
- Hooks (nested): `{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type":"command","command":"rtk hook claude"}]}]}}`.
- Plugins: `~/.claude/plugins/installed_plugins.json`:
  `{"version":2,"plugins":{"name@mp":[{"scope":"user"|"project"|"local","projectPath":"/abs","installPath":"/abs/cache/mp/name/ver","version":"..."}]}}`.
  Entries with `scope: "user"` are global; others belong to `projectPath`.
- Marketplaces: `~/.claude/plugins/known_marketplaces.json`:
  `{"mp": {"source": {"source":"github","repo":"org/repo"}, "installLocation": "/abs/.claude/plugins/marketplaces/mp"}}`.
- Plugin contents (display children): `<installPath>/skills/*/SKILL.md`,
  `<installPath>/hooks/hooks.json`, `<installPath>/agents/*.md`, `<installPath>/commands/*.md`.
- Loose skills: `~/.claude/skills/<name>/SKILL.md`, `<proj>/.claude/skills/<name>/` (often
  symlinks into `~/.agents/skills/<name>` made by `npx skills`).

### Codex
- Config: `~/.codex/config.toml`, `<proj>/.codex/config.toml`.
  `[marketplaces.<mp>] source_type = "git", source = "https://github.com/org/repo.git"`,
  `[plugins."name@mp"] enabled = true`.
- Plugin cache: `~/.codex/plugins/cache/<mp>/<name>/<version>/`.
- Hooks: `~/.codex/hooks.json`, `<proj>/.codex/hooks.json` — same nested layout as Claude.
  Also inline in `config.toml`: `[[hooks.SessionStart]] matcher = "startup|resume"` +
  `[[hooks.SessionStart.hooks]] type = "command", command = "..."` (or inline arrays,
  `SessionStart = [{ matcher = "x", hooks = [{ command = "y" }] }]`). Non-array values under
  `hooks` (the `[hooks.state."<key>"]` trust tables) are not events.
- Loose skills: `~/.codex/skills/` (skip the `.system` dir), `~/.agents/skills/`,
  `<proj>/.agents/skills/`, `<proj>/.codex/skills/`.

### Cursor
- Hooks (flat): `~/.cursor/hooks.json`, `<proj>/.cursor/hooks.json`:
  `{"version":1,"hooks":{"sessionStart":[{"command":"bd cursor-hook sessionStart"}]}}`.
- Loose skills: `~/.cursor/skills/`, `<proj>/.cursor/skills/`. (Skip `~/.cursor/skills-cursor`,
  it is Cursor's built-in set.)
- Plugins: not supported in MVP.

### npx `skills` (vercel-labs/skills)
All of this lives in `npx_skills`; discovery, install and the Codex adapter go through it.
- Canonical store: `~/.agents/skills/<name>/`, project `<proj>/.agents/skills/<name>/`.
  Agent dirs hold symlinks to it.
- Lock: global `~/.agents/.skill-lock.json`, project `<proj>/skills-lock.json`:
  `{"version":3,"skills":{"<name>":{"source":"org/repo","sourceType":"github",...}}}`.
- Install: `npx -y skills add <source> --skill <a> --skill <b> -a claude-code -a codex -a cursor [-g] -y`.
  Agent ids: claude -> `claude-code`, codex -> `codex`, cursor -> `cursor`.

## Ids and grouping

- Skill: `skill:<canonical path>`; group = lock `source` or `"local"`; source = same.
- Hook group: `hook:<file>:<group>`; name = `hook_group(cmd)`; group = `"hooks"`;
  children = `"<Event> [<matcher>]: <command>"` (display only).
- Plugin: `plugin:<agent>:<scope>:<name@mp>`; name = `name@mp`; group = `mp`.
- Marketplace: `marketplace:<agent>:<name>`; group = name; source = `org/repo` or URL.

## Removal semantics (complete removal, no backups)

- **Skill**: remove every agent-dir entry (symlink or dir) that resolves to the canonical
  dir, then the canonical dir **only if it lives inside a known skill dir** (never delete a
  symlink target outside skill dirs — that is the user's source checkout), then
  `JsonRemoveKey ["skills", name]` on the scope's lock file.
- **Hook group**: one `RemoveHook { event, command }` per handler; `apply_edit` hands it to
  `hooks::remove_hook` with the format from the file's extension (`.toml` keeps formatting,
  JSON keeps key order). Empty groups/events are pruned.
- **Claude plugin**: `JsonRemoveKey ["enabledPlugins", id]` in the scope's settings
  (project scope: both `settings.json` and `settings.local.json`) and
  `JsonRemoveArrayItems ["plugins", id]` in `installed_plugins.json`, dropping records with
  `scope: "user"` (global) or `projectPath: <project>` (project). Those fields come from
  `claude::record_fields` alone; the adapter matches them against parsed records with the
  same `action::fields_match` the edit uses. When that drops the id's last
  install record (no other scope/project still has one), also `RemovePath` of
  `~/.claude/plugins/cache/<mp>/<name>` and `~/.claude/plugins/data/<name>-<mp>` (both or
  neither, per the segment rule below).
- **Claude marketplace** (global only): its plugins' removals (plus each plugin's cache/data
  dirs when every record is removed), `JsonRemoveKey [mp]` in `known_marketplaces.json`,
  `JsonRemoveKey ["extraKnownMarketplaces", mp]` in settings, and `RemovePath(installLocation)`
  **only if it is exactly one segment under `~/.claude/plugins/marketplaces/`** — a
  `directory` source's location is the user's own checkout and is never deleted.
- **Codex plugin** (ids without `@` are skipped): `TomlRemoveKey ["plugins", id]` +
  `TomlRemoveKey ["hooks", "state", key]` for every `hooks.state` key starting `"<id>:"` +
  `RemovePath(cache/<mp>/<name>)`.
- **Codex marketplace**: its plugins' removals + `TomlRemoveKey ["marketplaces", mp]` +
  `RemovePath(~/.codex/plugins/cache/<mp>)`.
- Codex `RemovePath`s are global only; the TOML edits still run when a path is rejected.
- **Removal safety, one rule.** A `RemovePath` holds an `action::RemovalTarget`, whose only
  constructor `RemovalTarget::new(root, segments)` joins segments onto an owned root and
  returns `None` (the adapter plans no removal) unless there is at least one segment and each
  is one plain UTF-8 name: non-empty, not `.`/`..`, no `/` or `\`. Config-supplied names go
  in as separate segments, so `a/b` is rejected rather than split. Roots: Claude
  `plugins/cache`, `plugins/data`, `plugins/marketplaces`; Codex `plugins/cache`; skills the
  agent skill dir holding the link, and the owned skill dir of the same scope holding the
  canonical dir.
- `execute` writes all edits or none (a failed rename restores files already replaced, best
  effort), then runs `RemovePath`s in plan order; a failed removal errors naming the paths
  already removed (no rollback). Before deleting it re-checks the target is strictly inside
  its root and skips (never errors) a target that is not, or whose parent or grandparent is a
  symlink, so nothing is deleted through a link.
- `execute` validates every edited file re-parses before writing; after uninstall the CLI
  re-runs discovery and reports any selected id that is still present.

## Hook grouping (`hook_group`)

Split with shell-words (fallback: whitespace). Skip `VAR=value` assignments and wrappers
(`bash sh zsh node python python3 uv bun deno npx bunx pnpm exec env sudo command run if [`
etc.). For the first remaining token:
- contains `${CLAUDE_PLUGIN_ROOT}` -> `"plugin"`
- is a path under a hidden tool dir in home such as `/.orca/` or `/.superset/` (a `.<name>`
  segment that is not `.claude`, `.codex`, `.cursor`, `.agents`, `.config`, `.local`) -> `<name>`
- is a path -> file stem (`herdr-agent-state.sh` -> `herdr-agent-state`)
- else -> the token (`bd`, `rtk`).

## CLI

```
skillz list [QUERY|PATH] [-C PATH] [--all] [-g] [--skills] [--hooks] [--plugins] [--marketplaces] [-a <agent>]... [-i]
skillz uninstall|rm|delete [QUERY] [same filters] [-y]
skillz install <org/repo | url | path> [-g] [-a <agent>]... [--all | --skill a,b,*commit]... [-y]
```
- Scope: `PATH` only -> project at PATH; `-g` only -> global; neither -> both (project = cwd).
- Kind flags filter (singular aliases `--skill/--hook/--plugin/--marketplace` on list and
  uninstall). No kind flag = all kinds. `-a` filters by agent.
- `list` prints group headers with item counts; a QUERY (fuzzy, as in uninstall; a matched
  marketplace pulls in its whole group) or `--all` prints the expanded tree. A positional that
  is an existing directory is the project path, not a query; `-C` forces query mode. `-i` opens
  the collapsible tree TUI.
- `uninstall`: filter -> fuzzy match QUERY over `name group source` -> multi-select picker
  -> show plan -> confirm -> execute -> re-discover and verify.
- `install`: resolve source (local path, else `git clone --depth 1`), scan skills and
  `.claude-plugin/marketplace.json`. Skills are named by their `SKILL.md` frontmatter `name:`
  (what npx `--skill` matches); as in npx, skills without `name`/`description` are skipped,
  as are `.git`, `node_modules`, `dist`, `build` and `__pycache__` dirs. `--all`/`--skill` or no marketplace -> npx skills path
  with pattern matching + confirm; otherwise marketplace path for Claude
  (`claude plugin marketplace add <src>`, `claude plugin install <p>@<mp> --scope user|project`).

## TUI

ratatui + crossterm. Widgets are pure state machines tested without a terminal:
- `tui::select`: list with `[x]` checkboxes; `↑↓/jk` move, `space` toggle, `a` toggle all
  visible, `/` enters filter (typed text fuzzy-filters; `esc` leaves filter), `enter` accept,
  `esc`/`q` cancel.
- `tui::tree_view`: collapsible tree; `↑↓/jk` move, `→/l/enter/space` expand/toggle,
  `←/h` collapse (or jump to parent), `/` filter keeps matching nodes and their ancestors,
  `q/esc` quit.
