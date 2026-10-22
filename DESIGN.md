# skillz design

Rust CLI (clap, ratatui) that lists, installs and completely removes skills, plugins,
marketplaces, hooks, MCP servers and subagents for Claude Code, Codex and Cursor, globally (`~`)
or per project, and loads named sets of them (profiles) into projects. Commands and keys are in
the README; this file is the structure behind them.

## Principles

- **Discovery is read-only and plans removal up front.** Each discovered `Augmentation`
  carries `removal: Vec<Action>`. Uninstall = concatenate the selected plans, execute.
- **A profile is the same machinery run forwards.** `provision` turns entries into install
  `Action`s; `apply` compares the profile's entries with the live inventory (nothing is stored), installs
  what is missing and, only on request, reuses the removal plans for the rest.
- **Effects at the edges.** `action::apply_edit` is a pure `&str -> String`. Only
  `action::execute`, `runner::SystemRunner`, the `atomic` and `subagent_store` writers and `tui`
  touch the outside world.
- **Dependency injection where effects are.** `Env` (home/project roots, redirected config
  dirs), `CommandRunner` (external programs), `Prompter` (user interaction) and the adapter list
  are passed in (`cli::Deps`). Tests use temp dirs, fake runners and scripted prompters.
- **Open for extension.** New agent = a module implementing `AgentAdapter` + one line in
  `agents::registry()`. New augmentation storage = new `Edit` variant. New profile entry kind = a
  `profile::Entry` variant (+ `validate`, `spec`, `identity` arms) and a `provision` arm in the
  adapters that can take it.
- **Tests never assert on user-facing strings** (messages, error text, rendered labels).
  Assert on structure: ids, kinds, counts, `Action`s, parsed file contents, tree shape.
- Coverage target > 75% (`cargo llvm-cov`).

## Adding an agent

All agent knowledge lives in `src/agents/<name>/` (or `<name>.rs`). Steps:

1. Add a variant to `Agent` in `model.rs`, with its `npx_id` (the id `npx skills -a` uses) and
   `Display` line. It is the one closed list; clap and serde need it.
2. Implement `AgentAdapter` in the new module and add one line to `agents::registry()`.
   Required: `agent`, `skill_dirs`, `hook_files`, `discover`, `home_dir_names`. Optional
   (defaults say "not supported"): `install_plugins`, `provision`, `usage` + `tracks`,
   `context_costs`, `plugin_root_var`.
3. Nothing else changes. `tests/conformance.rs` runs a fake adapter through list, usage,
   prune, uninstall and install; if a new agent needs an edit anywhere else, that is a leak.

## Module map

Top level (`src/`):

| Module | Owns |
| --- | --- |
| `main`, `lib` | the binary calls `cli::main`; `lib` lists the modules |
| `model` | `Agent`, `Kind` (marketplace, plugin, skill, hook, mcp, subagent), `Scope`, `Installer`, `Origin` (link, version), `Augmentation` |
| `env` | `Env`: home, project, `CLAUDE_CONFIG_DIR`/`CODEX_HOME` redirects and the paths derived from them (`skillz_dir` = `~/.config/skillz`, `store_path` = `<skillz_dir>/store.json`) |
| `matcher` | `fuzzy_rank` (substring first, then fuzzy), match positions, `pattern_matches` (glob) |
| `npx_skills` | the npx `skills` format: store dir and lock path per scope, lock parsing (name -> `LockEntry`: source, `sourceUrl`, folder hash), `add` command line |
| `usage` | merges each adapter's `usage()` records into last use per skill/plugin |
| `cost` | always-on context tokens per skill/plugin, merged from each adapter's `context_costs()`; `list --json` adds `tokens` |
| `prune` | pure checks: duplicate loose skills, never-used and 7+ days unused skills/plugins |
| `remove` | removal as data: `candidates` (query + `--yes` rule), `Plan` (steps, owner), `run` (execute, re-read the inventory, `Outcome`); prints and prompts nothing |
| `update` | `steps(item, cwd)`: the tool command that updates an item (empty when it has none), `can_update`; `update::check`: the `UpdateCheck` trait, `NoCheck`, the Claude marketplace check and `spawn` (a worker thread sending outdated ids on a channel) |
| `provision` | `plan` / `plan_entries`: profile entries as install `Action`s. Skills go through `npx skills` for all agents at once; other entries ask each `AgentAdapter::provision`, and what an agent cannot take comes back as `unsupported` |
| `mcp` | MCP servers: `Server`/`Launch`, read from json (Claude) or toml (Codex), shown as `Augmentation::children` lines (secret names only, never values), read back by `snapshot` |
| `subagents` | subagent discovery (name from frontmatter/toml, source from the manifest) and the `provision` step that copies them |
| `subagent_store` | copying definitions from a directory or shallow clone, never over a foreign file, and the `.skillz-sources.json` manifest |
| `clone` | shallow clone of a source into a temp dir removed after use |
| `fswalk` | the one recursive directory walk (skips `.git`, `node_modules`, `dist`, `build`, `target`, `__pycache__`; never follows symlinks) |
| `json_edit` | parse, mutate, re-serialize JSON; an unchanged value returns the text verbatim |
| `atomic` | `write_atomic`: temp file in the same directory, then rename |
| `store` | the one global `store.json` (`Document`: `version`, `items`, `profiles`): `Store::read` and `Store::update` (read, change, atomic write under an advisory lock on `store.json.lock`); a file that does not parse, or has another version, is the typed `Corrupt` and is never written over |
| `redact` | hides credentials in urls, command lines and text (userinfo, query strings, secret-named flags and `NAME=` pairs, `Bearer`); `credential_count` |
| `runner` | `CommandRunner` trait (`run`, `run_quiet`, `launch`, `capture`, `with_envs`), `SystemRunner`, `run_checked` |
| `spinner` | the one-line stderr status for non-TUI commands |
| `tree` (+ `tree/table`) | display tree (`Node`) build, `COLUMNS`, `Widths` and the plain text table |
| `info` | the lines of the info modal, built before the terminal opens |
| `review` | typed plan, progress and outcome of a switch (`PlanView`, `Verdict`, `Outcome`), shared by screens and flows |
| `prompt` | `Prompter` trait (pickers, browser, workspace, plan, progress, outcome) and `AssumeYes` |

Directories:

| Module | Owns |
| --- | --- |
| `action` | `Action`, `Edit`; `apply_edit` (pure; `edit`, `json`, `toml`); `execute` (dedupe, commands, staged all-or-nothing file commit in `stage`, path removal); `RemovalTarget`; `files` |
| `hook_text` | hook handlers as text: `Format::of(file)`, `parse`, `remove_hook`, `add_hook`; one walker per layout |
| `hooks` | `discover` (one `Augmentation` per group), `plugin_hooks` (the hooks a plugin declares), `hook_group` + `GroupHints` (`group`), `provision` (a hook entry as an `AddHook` edit) |
| `skills` | loose-skill discovery (`discover`, `entry`), npx lock ownership (`ownership`), removal plans (`removal`) |
| `agents` | `AgentAdapter`, `registry`, `adapter_for`; `inventory` merges every adapter plus the shared skill/hook discovery |
| `agents/claude` | `files` (paths), `records` (plugin/marketplace json), `plugins`, `marketplace` (discover, install, provision), `mcp`, `removal`, `usage`, `context` (parses `claude -p "/context"`) |
| `agents/codex` | `config` (project-config guards), `plugins`, `mcp`, `removal`, `usage` + `timestamp` (session logs) |
| `agents/cursor` | skills and hooks only |
| `install` | `skillz install`: `install()` dispatch, `InstallRequest`, `SkillSelection`; `source` (local path or clone, rejects empty/option-like sources), `skills` (scan, selection, npx `add`) |
| `profile` | `Profile` (toml), `Entry` (skill, plugin, subagent, hook, mcp), `Store`, a facade over the `store` document, with `load_flat` for `extends`; `validate` (every value non-empty, not option-like), `spec` (the one-line typed form), `identity` (which entries are the same thing; `overlay`) |
| `library` | `Held`/`Presence` (which items are installed where) and `Cue` (manual, detected or recorded by skillz), shared by the Library tab and `library list`; `Catalog` of `LibraryItem`s (label, `Entry`, optional `Provenance`: link, engine = `Installer`, version, `recorded_at`); `import_installed` / `record_installed` turn installed `Augmentation`s into items with their `Origin`; `sync_installed` adds only what the catalog lacks (`Catalog::add_if_new`), with `Provenance::detected` set, and reports the rest as `Synced`; `Catalog::installable` swaps a skill's source for its recorded link |
| `sync` | `Scanner` (what is installed in both scopes; `InstalledScanner` reads every adapter), `sync_library` (scan result into the store under its lock, or a dry run), `spawn` (scan and sync on a thread, then call back) |
| `snapshot` | installed items -> a `Profile` through one collector per kind (`skills`, `plugins`, `subagents`, `hooks`, `mcp`); what cannot be reinstalled comes back as `Skipped` with a `Reason` |
| `apply` | `drift` (how many entries a profile lacks and how many installed items it does not want: the Project header, the Profiles badge and `profile list`), `standing` (pure: `compare` a profile's entries with the inventory into present / missing / extras; `loaded` picks the matching profile), `Applier::plan` (`Switch`: present, installs, and with `exact` the removals), `Applier::run` (remove via `remove`, then run the install steps) |
| `cli` | `args` (clap), `deps` (`Deps`), `scope` (scope and target rules), `run` (dispatch), one module per command group (`list`, `removal` for uninstall and reset, `prune`, `use_profile`, `isolate` for `run`, `profile`); the workspace side, a directory `library` and `profile_*` (the library and profile commands), `pick` (choose among a query's matches); `session` (what the workspace and the commands share: `Session`, `perform`, `change` (plan, progress, result of one change), `plan_view`, `notice`, and `Console`, the `Prompter` that writes a plan as lines and answers it with a yes/no or a dry run); `workspace` (`mod` the loop, `model` the snapshot the tabs show, `sync`); `cli::main` wires real dependencies |
| `tui` | below |

`tui` (ratatui + crossterm): `theme` (semantic colour roles), `glyphs` (Unicode and ASCII sets),
`keymap` (every binding, per scope), `help` (the `?` overlay), `filter`, `clipboard` (OSC 52),
`hygiene` (panic hook, signals, mouse and title restore), `shell` (owns the terminal across
screens), `app` (screen stack), `prompter` (`TuiPrompter`), `closure` and `dialogs` (small
screens), `plan`, `progress` and `result` (the three screens of a switch), `tree_view` (the picker and
browser list: `model`, `input`, `draw`, `marks`, `menu`), `workspace` (the tabbed workspace: `shell`
is the `Workspace` around its tabs, `tab` the `Tab` trait, `model` and `act` the shared data and
requests, one directory per tab kind: `installed` (the Project and Global tabs), `library`,
`profiles`), `widgets` (`outline`, `panel`,
`list`, `tabs`, `keybar`, `input`, `modal`, `confirm`, `toast`, `scroll`, `empty`).

## On-disk formats (observed on a real machine)

### Claude Code
- Settings: `~/.claude/settings.json` (global), `<proj>/.claude/settings.json`,
  `<proj>/.claude/settings.local.json`. Keys: `hooks`, `enabledPlugins` (`{"name@mp": bool}`),
  `extraKnownMarketplaces` (`{"mp": {...}}`).
- Hooks (nested): `{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type":"command","command":"rtk hook claude"}]}]}}`.
  Profile hooks are written to the global `settings.json` or the project's `settings.local.json`.
- Plugins: `~/.claude/plugins/installed_plugins.json`:
  `{"version":2,"plugins":{"name@mp":[{"scope":"user"|"project"|"local","projectPath":"/abs","installPath":"/abs/cache/mp/name/ver","version":"..."}]}}`.
  Entries with `scope: "user"` are global; others belong to `projectPath`.
- Marketplaces: `~/.claude/plugins/known_marketplaces.json`:
  `{"mp": {"source": {"source":"github","repo":"org/repo"}, "installLocation": "/abs/.claude/plugins/marketplaces/mp"}}`.
- Plugin contents (display children): `<installPath>/skills/*/SKILL.md`,
  `<installPath>/hooks/hooks.json`, `<installPath>/agents/*.md`, `<installPath>/commands/*.md`.
  Hooks a plugin declares are `hooks/hooks.json`, `hooks.json`, and the `hooks` of
  `.claude-plugin/plugin.json` (a path, a list of paths, or the hooks inline). Codex reads the same
  files from the newest cached version, with `.codex-plugin/plugin.json` as the manifest.
- Loose skills: `~/.claude/skills/<name>/SKILL.md`, `<proj>/.claude/skills/<name>/` (often
  symlinks into `~/.agents/skills/<name>` made by `npx skills`).
- MCP servers: `mcpServers` in `~/.claude.json` (user scope), the same key under
  `projects.<abs path>` there (local scope), and `<proj>/.mcp.json` (project scope, shared).
  With `CLAUDE_CONFIG_DIR` set, `.claude.json` lives inside it.
- Subagents: `~/.claude/agents/*.md`, `<proj>/.claude/agents/*.md` (frontmatter `name:`).

### Codex
- Config: `~/.codex/config.toml`, `<proj>/.codex/config.toml`.
  `[marketplaces.<mp>] source_type = "git", source = "https://github.com/org/repo.git"`,
  `[plugins."name@mp"] enabled = true`, `[mcp_servers.<name>]`.
- Plugin cache: `~/.codex/plugins/cache/<mp>/<name>/<version>/`.
- Hooks: `~/.codex/hooks.json`, `<proj>/.codex/hooks.json` — same nested layout as Claude.
  Also inline in `config.toml`: `[[hooks.SessionStart]] matcher = "startup|resume"` +
  `[[hooks.SessionStart.hooks]] type = "command", command = "..."` (or inline arrays,
  `SessionStart = [{ matcher = "x", hooks = [{ command = "y" }] }]`). Non-array values under
  `hooks` (the `[hooks.state."<key>"]` trust tables) are not events.
- Loose skills: `~/.codex/skills/` (skip the `.system` dir), `~/.agents/skills/`,
  `<proj>/.agents/skills/`, `<proj>/.codex/skills/`.
- Subagents: `~/.codex/agents/*.toml`, `<proj>/.codex/agents/*.toml` (`name = "..."`).
- Usage: tool calls in session logs under `sessions` and `archived_sessions`.

### Cursor
- Hooks (flat): `~/.cursor/hooks.json`, `<proj>/.cursor/hooks.json`:
  `{"version":1,"hooks":{"sessionStart":[{"command":"bd cursor-hook sessionStart"}]}}`.
- Loose skills: `~/.cursor/skills/`, `<proj>/.cursor/skills/`. (Skip `~/.cursor/skills-cursor`,
  it is Cursor's built-in set.)
- Plugins, MCP servers, subagents: not discovered or provisioned.

### npx `skills` (vercel-labs/skills)
All of this lives in `npx_skills`; discovery, install and the Codex adapter go through it.
- Canonical store: `~/.agents/skills/<name>/`, project `<proj>/.agents/skills/<name>/`.
  Agent dirs hold symlinks to it.
- Lock: global `~/.agents/.skill-lock.json`, project `<proj>/skills-lock.json`:
  `{"version":3,"skills":{"<name>":{"source":"org/repo","sourceType":"github",...}}}`.
- Install: `npx -y skills add <source> --skill <a> --skill <b> -a claude-code -a codex -a cursor [-g] -y`.
  Agent ids come from `Agent::npx_id`: claude -> `claude-code`, codex -> `codex`, cursor -> `cursor`.

### skillz's own files (`~/.config/skillz/`)
- `store.json`: `{"version":1,"items":[{"label","entry","provenance"}],"profiles":[...]}`. An item's
  `entry` and a profile's `entry` list use the profile `Entry` (tagged by `kind`: `skill`,
  `plugin`, `subagent`, `hook`, `mcp`). A profile is `name`, `description`, `extends`, `agents`
  (default `["claude","codex"]`) and `entry`; `skillz profile show` prints it as TOML (examples
  in the README). `provenance` is `{"link","engine","version","recorded_at"}` and is absent for
  items typed in. Where each value comes from: a skill's link and version are the lock entry's
  `sourceUrl` and `skillFolderHash` (project lock: `computedHash`); a Claude plugin's link is its
  marketplace source and its version is `installed_plugins.json` `version`; a Codex plugin's
  version is the newest directory in its cache. Hooks, MCP servers and subagents carry the
  installer only. Nothing can install a pinned revision: `npx skills add`, `claude plugin install`
  and `codex plugin add` take no version or ref option, so the version is informational.
- `store.json.lock`: the file the advisory lock is taken on.
- `envs/<profile>/{claude,codex}`: the config dirs `skillz run` points the command at.
- `.skillz-sources.json` inside an agent's `agents/` dir: `{"<file>": "<source>"}` for subagents
  skillz copied.

## Installer

Every item records what installed it (`Installer`): `Npx` (skill with a `.skill-lock.json`
entry), `ClaudeCli` / `CodexCli` (plugins, marketplaces and MCP servers in that agent's own
records), or `Local` (a skill without a lock entry, e.g. copied from a local path, hooks and
subagents). Removal uses this to pick the tool's own uninstall.

## Ids and grouping

- Skill: `skill:<canonical path>` (plus `:project` for a project item whose canonical dir a
  global item shares, e.g. a project link into `~/.agents/skills`); group = lock `source` or
  `"local"`; source = same.
- Hook group: `hook:<file>:<group>`; name = `hook_group(cmd)`; group = `"(unknown) hooks"`
  (`UNOWNED_HOOKS`): these are the hooks no plugin declares;
  children = `"<Event> [<matcher>]: <command>"` (display only); `Augmentation::hooks` holds the
  handlers as data.
- Plugin: `plugin:<agent>:<scope>:<name@mp>`; name = `name@mp`; group = `mp`. The hooks it declares
  are in `Augmentation::hooks` and in `children` as `"hook: <Event> [<matcher>]: <command>"`. They
  have no removal of their own (removing the plugin removes them) and are not hook items: the
  snapshot's hook collector passes over plugins, so the plugin entry alone covers them. A hook in
  a settings file whose command points into a plugin directory stays unowned: a path is not proof
  that the plugin added it.
- Marketplace: `marketplace:<agent>:<name>`; group = name; source = `org/repo` or URL.
- MCP server: `mcp:claude:<scope>:<claude scope>:<name>` or `mcp:codex:<scope>:<name>`;
  group = `"mcp"`; children = launch lines and `needs: NAME` for secrets.
- Subagent: `subagent:<agent>:<scope>:<file>`; group and source = the manifest's source, else
  `"local"`.

## Profiles and switching

- **Entries to steps.** `provision::plan_entries` makes one `npx skills add` per skill entry for
  all its agents; for every other kind it asks each of the entry's agents (`AgentAdapter::provision`)
  and records `Unsupported { entry, agent }` for a `None`. Claude: plugins via the Claude CLI
  (scope `user`/`local`), MCP via `claude mcp add --scope user|local`, hooks as an `AddHook` edit,
  subagents as `Action::CopyAgents`. Codex: plugins and MCP global only (`codex plugin ...`,
  `codex mcp add`; no SSE), hooks in `hooks.json`, subagents as `.toml` copies. Cursor: skills only.
- **`extends`.** `Store::load_flat` folds bases in order with `identity::overlay`: the child's
  entries replace inherited ones with the same key and go last; a partly overlapping skill or
  subagent entry keeps its other names; each base's entries are resolved with its own agents.
- **Standing.** `apply::compare` turns each wanted (resolved) entry and each installed item into
  claims (an identity key from `profile::identity` per agent; installed items go through
  `snapshot`). An entry is `present` when all its claims are installed, otherwise `missing` is the
  entry cut down to the names still lacking. An installed item is an `extra` unless one of its
  claims is wanted (a marketplace: unless a wanted plugin comes from it); `sourced` is false when
  no profile could reinstall it. A project matches a profile when nothing is missing; `loaded`
  picks the matching profile with the most claims in place. Nothing about this is persisted.
- **Switch.** `Applier::plan` reads the inventory, compares, and plans the install steps of the
  missing entries. With `exact` the extras become removals, planned with their normal removal
  `Action`s via `remove::Plan::from_owned`; unsourced ones are flagged in the plan.
- **Run.** `Applier::run` removes first, then executes the install steps. If the inventory cannot be
  read nothing is planned.
- **Isolation.** `skillz run` is an additive switch to scope `Global` of an `Env` whose `claude_config` and
  `codex_home` are `envs/<profile>/...`, followed by `CommandRunner::launch`.
- **Workspace.** `Workspace::handle` (pure) returns an `Act`; `cli::workspace` performs it as a
  `Change` (install an entry, remove an installed item, update, apply a profile), each a `Switch`
  from the same planner (additive only). `Act::Change` and `Act::Update` carry the `Scope` they
  work in, which becomes the `Applier`'s scope, the plan heading, the inventory read before and
  after, and so the npx `-g`, Claude `--scope user` and Codex user config of the install steps;
  profiles apply to the project only. `change` shows the plan screen, then progress, then the result
  (retry replans). One directory (cwd or `skillz PATH`) is the only project and nothing is stored
  for it: the Project and Global tabs are derived from the native config, the library and the profiles. After
  each install (`install`, `use`, workspace changes) `cli::recording` adds the installed items
  that are new or changed since before the run to the library, with their provenance, then syncs everything installed in both scopes; a picked
  library item is installed from its recorded link (`Catalog::installable`).

## One implementation for the workspace and the commands

A workspace feature is a command: `skillz library` (`list`, `add`, `remove`, `install`),
`skillz profile` (`list`, `status`, `new`, `add`, `remove`, `save`, `delete`), `use [--exact]`,
`list [--available] [--json [--check]]`, `uninstall` and `update` with `--check`; the table is in
the README. A command opens a `Session` (the project, the store, the profiles) over `Deps` whose
prompter is a `Console`, and calls the session's own `library_add`, `library_remove`,
`create_profile`, `set_entries`, `delete_profile` or `change`, the functions the workspace's `Act`s
reach through `perform`; there is no second copy of the planning or the steps. `Console::plan`
prints the plan as lines and returns `Verdict::DryRun` for `--dry-run` or the answer to
`Proceed?` (`--yes` answers it); a `Notice::Failed` becomes the command's error and so a non-zero
exit. The comparison (`apply::drift`, `Standing`), `library::Held` and `Cue` are the same
functions the tabs draw from. Only navigation (movement, filter, fold, help, tab keys) is
workspace-only. `use` and `update` predate the session and still plan through `Applier` themselves.

## Library sync

The library is the master record of everything seen, so syncing is additive: `sync_installed`
appends installed items the catalog does not know (`add_if_new` leaves a known item, its label and
its provenance as they are) and nothing in the sync path removes. Removal from the library is the
explicit `LibraryRemove` act only. Found items carry `Provenance::detected`; items recorded by an
install do not, and a detected item that skillz installs later is rewritten without the flag.

A hand-made item has no recorded source, so discovery gives it a local one in `Origin::link`: the
directory holding the skill (`skills::local`; only when npx would see it, `skill_name`, and the
resolved skill lies inside the home, project or the agents' config dirs, so a symlink out is not
followed) or the directory holding the subagent file. A renamed skill (frontmatter `name` differing
from its directory, which is what `--skill` matches) notes it as a `skill: <name>` child. The skill
and subagent collectors take such an item with `Installer::Local` as an entry sourced by that
directory, so sync, `profile save` and the library agree; reinstalling is
`npx skills add <dir> --skill <name>` (verified against npx: a directory source is searched
recursively and `--skill` matches the frontmatter name). Only what has no directory or needs
secrets is still `Skipped`.

Scanning never holds the store: `sync::Scanner` reads the inventory first, then `sync_library`
takes the store lock for the merge alone. The workspace starts `sync::spawn` on a thread (the
`Scanner` is `None` with `SKILLZ_NO_SYNC`), whose callback sends a `tui::workspace::Synced` (the new
library, a toast only when something was added or left out for secrets, or a quiet failure toast)
over a channel. `Workspace::draw` takes finished syncs, reloads the tabs from its last `Model` with
the new library and toasts. `skillz sync` runs the same merge over `Scene::gather`, and
`cli::recording` runs it after every recorded change. The Library tab derives per-row `Presence`
(project, global) from the model's installed lists with `snapshot` and `same_thing`.

## Updating

`update::steps` maps `(Installer, Kind)` to the command of the tool that installed the item, as
`Action::Run` steps, so the plan popup, `--dry-run` and `Applier::run_each` (one failure does not
stop the rest; failures are reported by item name) work as for installs. A `Switch` carries them
as `updates`. What each tool supports was read off its `--help`:

| Item | Tool | Update |
| --- | --- | --- |
| skill (npx) | `npx skills` | `skills update [skills...] [-g or -p] [-y]` (alias `upgrade`): updates to the latest version |
| Claude plugin | `claude plugin` | `plugin update <plugin>` (a restart of Claude applies it) |
| Claude marketplace | `claude plugin` | `plugin marketplace update [name]` |
| Codex marketplace | `codex plugin` | `plugin marketplace upgrade [name]` (remote marketplaces only) |
| Codex plugin | `codex plugin` | no update command (`add`, `list`, `marketplace`, `remove` only): `marketplace upgrade <mp>` then `plugin add <id>`. That `add` over an installed plugin replaces it was not verified |
| subagent with a recorded source | skillz | copied again from the source (`CopyAgents`) |
| hook, MCP server, hand-made skill, subagent without a source | none | no update; `u` ignores them |

After a run, the usual recording path (`cli::recording`) compares the inventory before and after,
so an item whose lock hash or version changed gets its provenance in the library rewritten.

**Live progress.** `Applier::run_each` reports every item starting and ending (`review::ItemEvent`)
besides every step. While a selection or update runs, `Prompter::live` hands the terminal to a
painter thread (`tui::live`) that redraws the shared `Workspace` (an `Arc<Mutex<_>>`) every
100 ms; the apply stays on the calling thread, which feeds the events in, so the Project and Global tabs'
`Statuses` turn them into spinner, tick or cross on the rows. Without a workspace (the CLI),
`live` is the plain progress gauge. The plan and result screens run on the calling thread
before and after, when the painter has given the terminal back.

**Update check.** The Project and Global tabs mark rows that have a newer version (bold, `*` after the
name). `cli::workspace` starts `update::check::spawn` on a worker thread over the installed items of both
scopes and the tab takes ids off the channel when it draws (the shell redraws on every 200 ms tick, so
results show without a key press). The cache is the `Workspace`'s, per session, never written to
disk; applying an update forgets the ids and checks those items again. A checker is a
`Deps::checker` (`NoCheck` in tests). Only one kind can be told without side effects: a Claude
marketplace is a clone, so `rev-parse HEAD` in the clone is compared with `ls-remote origin HEAD`
(read-only, `GIT_TERMINAL_PROMPT=0`; any failure reads as no update). Nothing else is checked:
`npx skills` has no read-only outdated command (its lock hash is a folder hash, not comparable
with a remote head), Claude plugins record a version that only the marketplace's manifest could
be compared with, and Codex records no revision of a marketplace. Those kinds show no marker.
`SKILLZ_NO_UPDATE_CHECK` (set to anything) turns the check off. `skillz update --check [--json]`
runs the same checker once, in the foreground.

## Removal semantics (complete removal, no backups)

- **Native tool first.** An item installed by a tool starts its plan with that tool's own
  uninstall as an `Action::Run` (`fallback: true`): `npx -y skills remove <name> -a … [-g]`,
  `claude plugin uninstall <id> --scope user|project|local`, `claude plugin marketplace
  remove <name>`, `claude mcp remove <name> --scope …`, `codex plugin remove <id>` /
  `codex plugin marketplace remove <name>` / `codex mcp remove <name>` (Codex: global items only,
  since its CLI edits the user config). `execute` runs every `Run`
  first, in plan order. A non-zero exit aborts the plan before any file is touched; a program
  that is not installed is skipped (reported in `Report::skipped`). The file steps below then
  finish whatever the tool left, and the final inventory check confirms the item is gone.
  Commands run through `CommandRunner::run_quiet` (stdout and stdin detached, stderr kept
  for errors) so their banners do not print over the one `⠋ removing <name>` spinner line
  (`spinner::Spinner`, stderr, only on a terminal) that `action::execute_with` drives.
  Names from config files that are empty or option-like (`-x`) never become command args
  (`Action::native`). Installs use `Action::required`: no fallback, a missing program is an error.

- **Skill**: remove every agent-dir entry (symlink or dir) that resolves to the canonical
  dir, then the canonical dir **only if it lives inside a known skill dir** (never delete a
  symlink target outside skill dirs — that is the user's source checkout), then
  `JsonRemoveKey ["skills", name]` on the scope's lock file. Only a canonical dir exactly one
  segment under the scope's `.agents/skills` store has a lock entry (source and edit), keyed
  by that segment, not by an agent-dir entry's name.
- **Hook group**: one `RemoveHook { event, command }` per handler; `apply_edit` hands it to
  `hook_text::remove_hook` with the format from the file's extension (`.toml` keeps formatting,
  JSON keeps key order). Empty groups/events are pruned.
- **Subagent**: `RemovePath` of the definition file in the agent's `agents/` dir and a
  `JsonRemoveKey [file]` on `.skillz-sources.json`.
- **MCP server**: the agent's `mcp remove` (Claude with the item's scope), then the key itself:
  `JsonRemoveKey` under `mcpServers` in `.claude.json` / `.mcp.json`, or `TomlRemoveKey
  ["mcp_servers", name]`.
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
  owned, so a project item never deletes through a dir symlinked into `~`); subagents the
  agent's `agents` dir.
- **Writing config files.** `execute` plans every edit in memory first (any failure returns
  before the filesystem is touched). `action::stage` then writes each edited file to a temp file
  beside the real one (symlinks resolved so a dotfile link stays a link; the original mode is
  kept, never wider) and renames them all into place, but only if no target changed since the
  plan was read (otherwise nothing is written). A failed rename restores files already replaced,
  best effort; edits naming one file through different paths are merged. A file the edit would
  leave unparsable is an error, not written.
- Then `execute` runs `RemovePath`s in plan order, checking each `RemovePathIfJsonAbsent`
  condition against the files as written (missing file = absent, unparsable = error); a failed
  removal errors naming the paths already removed (no rollback). Before deleting it re-checks
  the target is strictly inside its root and skips (never errors) a target that is not, or whose
  parent or grandparent is a symlink, so nothing is deleted through a link.
- After uninstall the CLI re-runs discovery and reports any selected id that is still present.
- **Redaction.** Everything that prints a command or a failure goes through `redact`
  (`Action::describe`, `execute` errors, `runner`, clone failures).
- **Secrets stay out of profiles.** `mcp::Server` keeps env var and header names only;
  `Server::secret_count` also counts a credential embedded in args or url, and `snapshot` skips
  such servers (`Reason::Secrets`).

## Hook grouping (`hook_group`)

Split with shell-words (fallback: whitespace). Skip options, `VAR=value` assignments and
wrappers (`bash sh zsh node python uv bun npx pnpm exec env sudo command run if [` etc., the
`WRAPPERS` list); a quoted script (`bash -c "rtk hook"`) is split again. For the first remaining
token:
- contains `${CLAUDE_PLUGIN_ROOT}` (any adapter's `plugin_root_var`) -> `"plugin"`
- is a path under a hidden tool dir in home such as `/.orca/` or `/.superset/` (a `.<name>`
  segment that is not an agent's own dir or `.config`/`.local`) -> `<name>`
- is a path -> file stem (`herdr-agent-state.sh` -> `herdr-agent-state`)
- else -> the token (`bd`, `rtk`).

## CLI

```
skillz [PATH]                            # the workspace for PATH or cwd (needs a terminal)
skillz list [QUERY|PATH] [-p PATH] [--all] [--json] [-g] [kind flags] [-a <agent>]... [-i]
skillz uninstall|rm|delete [QUERY] [-p PATH] [-g] [kind flags] [-a <agent>]... [-y] [--dry-run]
skillz reset [PATH] [-g] [kind flags] [-a <agent>]... [--keep NAME]... [--dry-run | -f/--force]
skillz prune [PATH] [-g] [kind flags] [-a <agent>]... [--dry-run | -f/--force]
skillz install|add <org/repo | url | path> [-p PATH] [-a <agent>]... [--all | --skill 'a,b,*commit']... [-y]
skillz use <profile> [--exact] [-p PATH] [-y] [--dry-run]
skillz run <profile> [-p PATH] [-y] -- <command>...
skillz profile list|show <name> | save <name> [--force] | delete <name>   (all take -p PATH)
```
Kind flags: `--skills --hooks --plugins --marketplaces --mcp --subagents` (singular aliases).
- Scope (`cli::scope::resolve`, only for list, uninstall, reset and prune; `install` and `update` take the project via `resolve_project`, or the user's installs with `-g`; `use`, `run` and `profile` always act on the project): `PATH` only -> project at PATH; `-g` only -> global; neither ->
  both (project = cwd). A project that is the home dir is global only. `use` and `profile save`
  take exactly one scope (`-g` or the project).
- No kind flag = all kinds. `-a` filters by agent.
- `list` prints group headers with item counts; when QUERY matches span several groups on a terminal it
  opens the browser (`--json` and pipes take all matches); a QUERY (fuzzy, as in uninstall; a matched
  marketplace pulls in its whole group) or `--all` prints the expanded tree. A positional
  starting with `.`, `..` or `/` (`.`, `./sub`, `/abs`) is the project path; any other word is
  a query, even if a directory has that name; `-p` forces query mode. `-i` opens the
  browser on everything listed. Browser selections either print (Show) or go through the
  same `remove` step as `uninstall` (Remove).
- `uninstall`: filter -> match QUERY (an exact name wins, else fuzzy over `name group source`)
  -> grouped picker -> show plan -> confirm -> execute -> re-discover and verify. `-y` needs
  exactly one exact name match.
- `reset`: remove every skill, plugin and marketplace the filters leave (hooks only with `--hooks`,
  MCP servers only with `--mcp`, subagents only with `--subagents`: they belong to other tools),
  minus `--keep` matches by name/group/source; a marketplace stays while any of its plugins is
  spared. Same plan, confirm and verify as `uninstall`.
- `prune`: report duplicates (a loose skill every one of whose agents also gets it from an
  installed plugin: global, or the skill's own scope), never-used and 7+ days unused skills
  and plugins (only items a usage source can see: not hooks, marketplaces, Cursor-only items
  or Codex plugins without skills). Duplicates always go into the removal; unused items only
  when picked. Then the same `remove` step as `uninstall`. `--force` picks and confirms
  everything (the `AssumeYes` prompter).
- `install`: always the project (cwd or `-p PATH`). Resolve
  source (local path relative to the cwd, not `-p`; else shallow clone), scan skills and
  `.claude-plugin/marketplace.json`. Skills are named by their `SKILL.md` frontmatter `name:`
  (what npx `--skill` matches); as in npx, skills without `name`/`description` are skipped,
  as are the dirs `fswalk` skips and option-like (`-x`) skill/plugin/marketplace names.
  `--all`/`--skill` or no marketplace -> npx skills path with pattern matching + confirm;
  otherwise marketplace path for Claude (`claude plugin marketplace add <src>`,
  `claude plugin install <p>@<mp> --scope user|project`), then the skill picker for the other
  selected agents.
- `use`: load the profile (`load_flat`), `Applier::plan`, print `keep`/`remove`/`install`/`skip`
  lines and the steps, stop on `--dry-run`, ask `Proceed?` (skipped by `-y`), run with a
  `switching` spinner, and fail if something removed is still installed.
- `run`: `use` into the isolated global config (above), then `launch` the command in the
  project and return its exit code.
- `profile save` refuses an existing name without `--force`.

## TUI

ratatui + crossterm. Screens are built from `tui::widgets` and drawn with an injected `Theme`
(`theme.rs`): semantic roles, never colours. `Theme::detect` reads `SKILLZ_THEME`
(`dark|light|mono`), else `NO_COLOR`/`TERM=dumb` (mono), else `COLORFGBG`; `Glyphs::detect` reads
`SKILLZ_ASCII`, `TERM` (`linux`, `dumb`) and the locale for the ASCII symbol set;
`SKILLZ_NO_MOUSE` (set to anything) turns mouse capture off. All three are read once in
`cli::main`. `tui::shell` enters the terminal on the first screen and keeps it while a `Hold` is
out, so several screens in a row (workspace, plan, progress, result) do not flicker. Commands run
by a switch have their output captured so they cannot draw over it.

Keys live in one table, `keymap::BINDINGS`: each row has a scope (`Common`, `Workspace`, `Project`,
`Tree`, `Filter`), an `Action`, the chords, and how it reads on the key bar and in the `?` overlay.
Screens match on `Action`, never on keys; a scope's own binding wins over the common one, and the
filter scope passes no letters through. The key bar and `?` are generated from the table.

**Lists.** `widgets::outline` is the one collapsible outline: rows in preorder with parents, which
are open, a fuzzy filter and a scrolling cursor, with no idea what a row means. `tui::tree_view`
builds the picker and browser on it (`skillz list`, `uninstall`, `prune`, the install pickers; a
flat list is a tree with no children, `tree::flat`) and the Project and Global tabs build their sections on it,
so the Library and Profiles tabs can too. `TreeState::handle` is a pure state machine tested
without a terminal.
Rows are `tree::Node`s (`label`, `cells`: KIND, AGENTS, SCOPE, ITEMS, `info`, `item`);
`tree::Widths` lays the columns out and the plain `list` output (`tree::render`) uses the same
`Widths`, so the terminal list and the piped table match. JSON output is built from the items,
not the rows. Drawn as a title, a dim column header, an `X` column for marked rows (`-` for a
group with only some items marked), a key bar. Overlays: filter, help, the info modal (plain
data on `Node::info`, built before the terminal opens by `info::item_lines` and
`tree::attach_info`, so drawing never reads the filesystem) and, in the browser, the
Show / Remove… / Cancel menu. `enter` opens that menu for the marks (or the row under the cursor);
in a picker it accepts them (`Prompter::select_items` returns `Node::item` indices).

**Workspace.** `tui::workspace`: `Workspace` owns the tab bar (`tab`, `shift-tab`, `1`-`4`, from
`Scope::Workspace`), key bar, `?` help and toasts, and gives every `Tab` the same `Model` (the
directory, what it and the user have installed, the library, the readable profiles) after each act. A `Tab`
has `title`, `scopes` (its key scopes, which the key bar and help are generated from), `load`
(take a snapshot, keeping cursor, open groups and filter), `handle` (a key to an optional `Act`),
`draw`, and `captures_keys` / `prompt` for a filter or popup that must get every key. Adding a tab
is a directory under `workspace/`, an entry in `Workspace::standard`, and `Act` variants for what
it asks `cli::workspace` to do; the installed tabs are not touched. The tabs are Project, Global,
Library and Profiles. **Project and Global** are one `InstalledTab` (`installed/`) built with a
`Scope`; the scope picks which half of the `Model` it shows, whether it compares profiles and
whether it tags what is also installed globally, and is sent on in every `Act`. Its key scopes
are `Installed` (shared) plus `Project` (compare, apply profile) on the Project tab, resolved with
`keymap::resolve_in`. `rows` builds flat sections per kind from the scope's installed items,
then an `Available` header with a section per kind of the library items not installed in that
scope (`apply::holds` decides which are), `drift` is `apply::compare` against the chosen profile,
`picker` the compare-with popup, `header`, `keys`, `draw`. `space` on an item asks for
`Act::Change`; `cli::workspace::change` turns it into a one-entry or one-removal `Switch` and
shows the plan, progress and result before anything changes. Wheel scrolling works; there is no
other mouse handling.

**Switch screens.** `plan` shows the `PlanView` (keep `=`, remove `-`, install `+`, skip `~`;
danger frame if anything is removed) with keys to toggle the raw steps and a dry run; `progress`
shows a gauge and the log of started steps; `result` shows counts, failures (copyable through
OSC 52, retryable) and the steps that ran. These screens belong to the workspace; `skillz use`
prints the same plan as text and asks `Proceed?` as a `[y/N]` line (`Prompter::plan`'s default
does the same where there is no screen).
