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

## Adding an agent

All agent knowledge lives in `src/agents/<name>.rs`. Steps:

1. Add a variant to `Agent` in `model.rs` (and its `Display` line). It is the one closed list;
   clap and serde need it.
2. Implement `AgentAdapter` in the new module and add one line to `agents::registry()`.
   Required: `agent`, `npx_id`, `home_dir_names`, `skill_dirs`, `hook_files`, `discover`.
   Optional (defaults say "not supported"): `install_plugins`, `usage` + `tracks`,
   `plugin_root_var`.
3. Nothing else changes. `tests/conformance.rs` runs a fake adapter through list, usage,
   prune, uninstall and install; if a new agent needs an edit anywhere else, that is a leak.

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
| `tui/*` | the one list component: collapsible, markable tree with `/` filter |
| `prompt` | `Prompter` trait, `TuiPrompter`, `AssumeYes` |
| `runner` | `CommandRunner` trait, `SystemRunner` |
| `install` | install flow: `install()` dispatch, `InstallRequest`, `SkillSelection` |
| `install::source` | source fetch: reject empty/option-like sources, local path or `git clone --depth 1` into a temp dir, credential-redacted `run_checked` |
| `install::skills` | npx `skills` installer: `scan_available`, selection/patterns, runs `npx_skills::add_command` |
| `agents::claude_marketplace` | Claude's `AgentAdapter::install_plugins`: reads `.claude-plugin/marketplace.json`, runs `claude plugin marketplace add` + `plugin install`. `install` asks each agent in turn and sends agents that return `Unsupported` through `npx skills` |
| `usage` | merges each adapter's `usage()` records into last use per skill/plugin. Sources live in the adapters: Claude `~/.claude.json` `skillUsage`/`pluginUsage`, Codex session logs (tool calls reading a `SKILL.md`) |
| `cost` | always-on context tokens per skill/plugin, merged from each adapter's `context_costs()`: Claude parses `claude -p "/context"` (exact, per skill and agent; built-in and claude.ai skills left out), others estimate from `SKILL.md` frontmatter (~3.5 chars/token). `list --json` adds `tokens` |
| `prune` | pure checks: duplicate loose skills, never-used and 7+ days unused skills/plugins |
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

## Installer

Every item records what installed it (`Installer`): `Npx` (skill with a `.skill-lock.json`
entry), `ClaudeCli` / `CodexCli` (plugins and marketplaces in that agent's own records), or
`Local` (a skill without a lock entry, e.g. copied from a local path, and hooks). Removal uses
this to pick the tool's own uninstall (see the native-removal epic).

## Ids and grouping

- Skill: `skill:<canonical path>` (plus `:project` for a project item whose canonical dir a
  global item shares, e.g. a project link into `~/.agents/skills`); group = lock `source` or
  `"local"`; source = same.
- Hook group: `hook:<file>:<group>`; name = `hook_group(cmd)`; group = `"hooks"`;
  children = `"<Event> [<matcher>]: <command>"` (display only).
- Plugin: `plugin:<agent>:<scope>:<name@mp>`; name = `name@mp`; group = `mp`.
- Marketplace: `marketplace:<agent>:<name>`; group = name; source = `org/repo` or URL.

## Removal semantics (complete removal, no backups)

- **Native tool first.** An item installed by a tool starts its plan with that tool's own
  uninstall as an `Action::Run` (`fallback: true`): `npx -y skills remove <name> -a … [-g]`,
  `claude plugin uninstall <id> --scope user|project|local`, `claude plugin marketplace
  remove <name>`, `codex plugin remove <id>` / `codex plugin marketplace remove <name>`
  (Codex: global items only, since its CLI edits the user config). `execute` runs every `Run`
  first, in plan order. A non-zero exit aborts the plan before any file is touched; a program
  that is not installed is skipped (reported in `Report::skipped`). The file steps below then
  finish whatever the tool left, and the final inventory check confirms the item is gone.
  Names from config files that are empty or option-like (`-x`) never become command args
  (`Action::native`).

- **Skill**: remove every agent-dir entry (symlink or dir) that resolves to the canonical
  dir, then the canonical dir **only if it lives inside a known skill dir** (never delete a
  symlink target outside skill dirs — that is the user's source checkout), then
  `JsonRemoveKey ["skills", name]` on the scope's lock file. Only a canonical dir exactly one
  segment under the scope's `.agents/skills` store has a lock entry (source and edit), keyed
  by that segment, not by an agent-dir entry's name.
- **Hook group**: one `RemoveHook { event, command }` per handler; `apply_edit` hands it to
  `hooks::remove_hook` with the format from the file's extension (`.toml` keeps formatting,
  JSON keeps key order). Empty groups/events are pruned.
- **Claude plugin**: `JsonRemoveKey ["enabledPlugins", id]` in the scope's settings
  (project scope: both `settings.json` and `settings.local.json`) and
  `JsonRemoveArrayItems ["plugins", id]` in `installed_plugins.json`, dropping records with
  `scope: "user"` (global) or `projectPath: <project>` (project), as `claude::record_fields`
  selects. Always also `RemovePathIfJsonAbsent` of `~/.claude/plugins/cache/<mp>/<name>` and
  `~/.claude/plugins/data/<name>-<mp>` (both or neither, per the segment rule below) on
  `installed_plugins.json` `["plugins", id]`: the dirs go only once the committed edits leave
  no install record, so selecting every scope of a plugin removes them and selecting one of
  several keeps them.
- **Claude marketplace** (global only): its plugins' removals (their cache/data dirs under the
  same condition), `JsonRemoveKey [mp]` in `known_marketplaces.json`,
  `JsonRemoveKey ["extraKnownMarketplaces", mp]` in settings, and `RemovePath(installLocation)`
  **only if it is exactly one segment under `~/.claude/plugins/marketplaces/`** — a
  `directory` source's location is the user's own checkout and is never deleted.
- **Codex plugin** (ids without `@` are skipped): `TomlRemoveKey ["plugins", id]` +
  `TomlRemoveKey ["hooks", "state", key]` for every `hooks.state` key starting `"<id>:"` +
  `RemovePath(cache/<mp>/<name>)`.
- **Codex marketplace**: its plugins' removals + `TomlRemoveKey ["marketplaces", mp]` +
  `RemovePath(~/.codex/plugins/cache/<mp>)`.
- Codex `RemovePath`s are global only; the TOML edits still run when a path is rejected. A
  global plugin keeps its cache while the current project's `config.toml` lists that id, and
  a marketplace keeps `cache/<mp>` while it lists any `<name>@<mp>` (an unreadable project
  config keeps every cache). Other projects' configs are not visible.
- **Removal safety, one rule.** A `RemovePath` holds an `action::RemovalTarget`, whose only
  constructor `RemovalTarget::new(root, segments)` joins segments onto an owned root and
  returns `None` (the adapter plans no removal) unless there is at least one segment and each
  is one plain UTF-8 name: non-empty, not `.`/`..`, no `/` or `\`. Config-supplied names go
  in as separate segments, so `a/b` is rejected rather than split. Roots: Claude
  `plugins/cache`, `plugins/data`, `plugins/marketplaces`; Codex `plugins/cache`; skills the
  agent skill dir holding the link, and the owned skill dir of the same scope holding the
  canonical dir (a project skill dir whose canonical path is outside the project is not
  owned, so a project item never deletes through a dir symlinked into `~`).
- `execute` writes all edits or none (a failed rename restores files already replaced, best
  effort; edits naming one file through different paths, e.g. a symlink, are merged), then
  runs `RemovePath`s in plan order, checking each `RemovePathIfJsonAbsent` condition against
  the files as written (missing file = absent, unparsable = error); a failed removal errors
  naming the paths already removed (no rollback). Before deleting it re-checks the target is strictly inside
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
skillz list [QUERY|PATH] [-p PATH] [--all] [--json] [-g] [--skills] [--hooks] [--plugins] [--marketplaces] [-a <agent>]... [-i]
skillz uninstall|rm|delete [QUERY] [same filters] [-y]
skillz reset [PATH] [same filters] [--keep NAME]... [--dry-run | -f/--force]
skillz prune [PATH] [same filters] [--dry-run | -f/--force]
skillz install <org/repo | url | path> [-g] [-a <agent>]... [--all | --skill 'a,b,*commit']... [-y]
```
- Scope: `PATH` only -> project at PATH; `-g` only -> global; neither -> both (project = cwd).
  A project that is the home dir is global only.
- Kind flags filter (singular aliases `--skill/--hook/--plugin/--marketplace` on list and
  uninstall). No kind flag = all kinds. `-a` filters by agent.
- `list` prints group headers with item counts; when QUERY matches span several groups on a terminal it
  opens the browser (`--json` and pipes take all matches); a QUERY (fuzzy, as in uninstall; a matched
  marketplace pulls in its whole group) or `--all` prints the expanded tree. A positional
  starting with `.`, `..` or `/` (`.`, `./sub`, `/abs`) is the project path; any other word is
  a query, even if a directory has that name; `-p` forces query mode. `-i` opens the
  browser on everything listed. Browser selections either print (Show) or go through the
  same `remove` step as `uninstall` (Remove).
- `uninstall`: filter -> fuzzy match QUERY over `name group source` -> grouped picker
  -> show plan -> confirm -> execute -> re-discover and verify.
- `reset`: remove every skill, plugin and marketplace the filters leave (hooks only with `--hooks`:
  they belong to other tools), minus `--keep` matches by name/group/source; a marketplace stays
  while any of its plugins is spared. Same plan, confirm and verify as `uninstall`.
- `prune`: report duplicates (a loose skill every one of whose agents also gets it from an
  installed plugin: global, or the skill's own scope), never-used and 7+ days unused skills
  and plugins (only items a usage source can see: not hooks, marketplaces, Cursor-only items
  or Codex plugins without skills). Duplicates always go into the removal; unused items only
  when picked. Then the same `remove` step as `uninstall`. `--force` picks and confirms
  everything (the `AssumeYes` prompter).
- `install`: project scope unless `-g` or the project is the home dir (then global). Resolve
  source (local path relative to the cwd, not `-p`; else `git clone --depth 1`), scan skills and
  `.claude-plugin/marketplace.json`. Skills are named by their `SKILL.md` frontmatter `name:`
  (what npx `--skill` matches); as in npx, skills without `name`/`description` are skipped,
  as are `.git`, `node_modules`, `dist`, `build` and `__pycache__` dirs, and option-like
  (`-x`) skill/plugin/marketplace names. `--all`/`--skill` or no marketplace -> npx skills
  path with pattern matching + confirm; otherwise marketplace path for Claude
  (`claude plugin marketplace add <src>`, `claude plugin install <p>@<mp> --scope user|project`),
  then the skill picker for the other selected agents.

## TUI

ratatui + crossterm. One list component, `tui::tree_view`, serves every list: the browser
(`skillz list`) and every picker (`uninstall`, `prune`, the install pickers; a flat list is a
tree with no children, `tree::flat`). It is a pure state machine tested without a terminal,
drawn through `tui::draw_list`: a title line, a dim column header, no border, an `X` column
for marked rows (`-` for a group with only some items marked), a key-hint footer. Rows are
`tree::Node`s (`label` plus `cells`: KIND, AGENTS, SCOPE, ITEMS); `tree::Widths` lays the
columns out, and the plain `list` output (`tree::render`) uses the same `Widths`, so the
terminal list and the piped table match. JSON output is built from the items, not the rows.
Keys: `↑↓/jk` move, `space` expand/collapse, `→/l` expand, `←/h` collapse (or jump to parent),
`x`/`tab` mark a group (all its items) or one item, `/` filter keeps matching nodes and their
ancestors, `i` opens the info modal (kind, scope, agents, group, source, installer, last used,
contents, and the commands removing it runs; a group shows its members), `esc`/`q`/`i`/`enter`
close it. Its text is plain data on `Node::info`, built before the terminal opens
(`info::item_lines`, `tree::attach_info`) so drawing never reads the filesystem, `q/esc` quit. In the browser `enter` opens the Show / Remove… / Cancel menu for
the marks (or the row under the cursor); in a picker `enter` accepts them
(`Prompter::select_items` returns `Node::item` indices). Rows map back to items through
`tree::Node::item`.
