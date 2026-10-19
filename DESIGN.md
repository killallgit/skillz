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
| `action` | `Action`, `Edit`, `apply_edit` (pure), `execute` (effects) |
| `hooks` | parse hook JSON, `hook_group`, `discover(file)` |
| `skills` | shared loose-skill discovery (npx `skills` aware) |
| `matcher` | `fuzzy_rank`, `pattern_matches` (glob) |
| `agents/*` | per-agent plugin/marketplace discovery + `skill_dirs` + `hook_files` |
| `tree` | display tree build + text render |
| `tui/*` | multi-select list, collapsible tree, `/` filter |
| `prompt` | `Prompter` trait, `TuiPrompter`, `AssumeYes` |
| `runner` | `CommandRunner` trait, `SystemRunner` |
| `install` | install flow, `scan_available` |
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
- Loose skills: `~/.codex/skills/` (skip the `.system` dir), `~/.agents/skills/`,
  `<proj>/.agents/skills/`, `<proj>/.codex/skills/`.

### Cursor
- Hooks (flat): `~/.cursor/hooks.json`, `<proj>/.cursor/hooks.json`:
  `{"version":1,"hooks":{"sessionStart":[{"command":"bd cursor-hook sessionStart"}]}}`.
- Loose skills: `~/.cursor/skills/`, `<proj>/.cursor/skills/`. (Skip `~/.cursor/skills-cursor`,
  it is Cursor's built-in set.)
- Plugins: not supported in MVP.

### npx `skills` (vercel-labs/skills)
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
- **Hook group**: one `JsonRemoveHook` per handler. Empty groups/events are pruned.
- **Claude plugin**: `JsonRemoveKey ["enabledPlugins", id]` in the scope's settings
  (project scope: both `settings.json` and `settings.local.json`) and
  `JsonRemovePluginInstall` in `installed_plugins.json`. Cache dir is left (Claude sweeps it).
- **Claude marketplace** (global only): its plugins' removals, `JsonRemoveKey [mp]` in
  `known_marketplaces.json`, `JsonRemoveKey ["extraKnownMarketplaces", mp]` in settings,
  `RemovePath(installLocation)`.
- **Codex plugin** (ids without `@` are skipped): `TomlRemoveKey ["plugins", id]` +
  `TomlRemoveKey ["hooks", "state", key]` for every `hooks.state` key starting `"<id>:"` +
  `RemovePath(cache/<mp>/<name>)`.
- **Codex marketplace**: its plugins' removals + `TomlRemoveKey ["marketplaces", mp]` +
  `RemovePath(~/.codex/plugins/cache/<mp>)`.
- Codex `RemovePath`s (global only) are emitted only when every name joined into the path is
  a single plain segment (non-empty, not `.`/`..`, no `/` or `\`); the TOML edits still run.
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
skillz list [PATH] [-g] [--skills] [--hooks] [--plugins] [--marketplaces] [-a <agent>]... [-i]
skillz uninstall|rm|delete [QUERY] [same filters] [-y]
skillz install <org/repo | url | path> [-g] [-a <agent>]... [--all | --skill a,b,*commit]... [-y]
```
- Scope: `PATH` only -> project at PATH; `-g` only -> global; neither -> both (project = cwd).
- Kind flags filter (singular aliases `--skill/--hook/--plugin/--marketplace` on list and
  uninstall). No kind flag = all kinds. `-a` filters by agent.
- `list` prints the tree; `-i` opens the collapsible tree TUI.
- `uninstall`: filter -> fuzzy match QUERY over `name group source` -> multi-select picker
  -> show plan -> confirm -> execute -> re-discover and verify.
- `install`: resolve source (local path, else `git clone --depth 1`), scan skills and
  `.claude-plugin/marketplace.json`. `--all`/`--skill` or no marketplace -> npx skills path
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
