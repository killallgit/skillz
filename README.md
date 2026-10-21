# skillz

List, install and completely remove skills, plugins, marketplaces, hooks, MCP servers and
subagents for Claude Code, Codex and Cursor, globally or per project, and keep named sets of them
(profiles) that you can load into a project and swap.

```sh
task install                     # release build, symlinked to ~/.local/bin/skillz
# or: cargo install --path .
```

`skillz` with no arguments opens the workspace (see [Workspace](#workspace)); everything it does
is also a command.

## Commands

| Command | Does |
| --- | --- |
| `list` (`ls`) | show what is installed as a tree, or browse it with `-i` |
| `uninstall` (`rm`, `delete`) | fuzzy match, pick, confirm, remove completely |
| `reset` | remove every skill, plugin and marketplace in scope |
| `prune` | find duplicate and unused skills and plugins, remove the ones picked |
| `install` (`add`) | install skills with `npx skills`, or Claude plugins from a marketplace |
| `use` | load a profile into a project (or globally), swapping out the one that is there |
| `run` | run a command against one profile in config dirs of its own |
| `profile` | `list` (`ls`), `show`, `save`, `delete` (`rm`) |
| `project` | `list` (`ls`), `add`, `remove` (`rm`) |

Kind filters on `list`, `uninstall`, `reset` and `prune`: `--skills --hooks --plugins
--marketplaces --mcp --subagents` (singular spellings work too; none given means all kinds).
`-a claude|codex|cursor` filters by agent (repeatable), `-g` means global only. Run
`skillz <command> --help` for every flag.

```sh
skillz list                      # global + current project, groups collapsed
skillz list matt                 # matches in several groups: open the browser
skillz list ponytail             # expand what matches (a marketplace brings its group)
skillz list --all                # expand everything
skillz list --hooks .            # project hooks only
skillz list -g --skills -a claude
skillz list -i                   # browser: expand, mark, enter -> Show / Remove…
skillz list --json | jq '.[].name'  # items + removal plans

skillz rm tdd                    # fuzzy match -> pick -> confirm -> remove
skillz rm --hook bd -a cursor    # every hook that runs `bd`, for Cursor
skillz rm caveman@caveman -y     # non-interactive: needs exactly one exact name match
skillz rm speckit --dry-run      # print the removal plan only

skillz reset                     # skills, plugins, marketplaces; --hooks, --mcp, --subagents add those kinds
skillz reset --keep caveman      # spare items by name, group or source
skillz reset --dry-run | --force
skillz prune                     # duplicates + unused 7+ days -> pick -> confirm -> remove
skillz prune --dry-run | --force

skillz install mattpocock/skills --skill 'tdd,*commit'   # glob match, confirm, npx skills add
skillz install mattpocock/skills --all
skillz install killallgit/killall-skills                 # has a marketplace: pick Claude plugins
```

Scope for `list`, `uninstall`, `reset` and `prune`: a `PATH` (or `-p PATH`) means that project only,
`-g` means global only, neither means both (project = cwd). `install`, `use`, `run`, `profile` and
the TUI always act on one project. A project that is your home directory counts as global. A `list` PATH must
start with `.`, `..` or `/` (`./sub`, not `sub`); any other word is a query.

## Profiles

A profile is `~/.config/skillz/profiles/<name>.toml`. Names are letters, digits, `-`, `_` and `.`
and do not start with `-` or `.`.

```toml
description = "frontend work"         # optional
extends = ["base"]                    # optional, see below
agents = ["claude", "codex"]          # default for entries that name none (this is the default)

[[entry]]
kind = "skill"                        # installed with `npx skills`
source = "mattpocock/skills"          # org/repo, git URL or local path
names = ["tdd", "grilling"]

[[entry]]
kind = "plugin"
source = "killallgit/killall-skills"  # what adds the marketplace
marketplace = "killallgit"
name = "killall-skills"
agents = ["claude"]

[[entry]]
kind = "subagent"
source = "msitarzewski/agency-agents" # a repo, or a directory relative to the project
names = ["frontend-developer", "ui-designer"]  # file names or the agents' own names
agents = ["claude"]

[[entry]]
kind = "mcp"
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
# or: url = "https://example.com/mcp", transport = "sse"   (transport: http (default) or sse)

[[entry]]
kind = "hook"
event = "SessionStart"
matcher = "startup|resume"            # optional; absent means all
command = "bd prime"
```

Every value must be non-empty and must not start with `-`; an MCP entry has exactly one of
`command` or `url`. Of the agents, Cursor only takes skills; Codex takes what its CLI and config
can (see below); anything an agent cannot take is shown as skipped in the plan, not as an error.

| Kind | Claude | Codex |
| --- | --- | --- |
| skill | `npx skills add`, all agents at once | same |
| plugin | `claude plugin marketplace add` + `plugin install`; project scope uses Claude's `local` scope, never the file the team shares | global only |
| subagent | `.md` files into `.claude/agents` (`~/.claude/agents` global) | `.toml` files into `.codex/agents` (`~/.codex/agents` global) |
| hook | `settings.json` (global) or `.claude/settings.local.json` (project) | `hooks.json` in the same scope |
| mcp | `claude mcp add`, scope `user` (global) or `local` (project) | `codex mcp add`, global only, no SSE |

Subagents are copied from the source and remembered in `.skillz-sources.json` beside them, so
`list --subagents` groups them by source, a snapshot can name the source, and an exact apply deletes
those files. skillz never overwrites a file it did not put there.

### Extending

`extends = ["base"]` loads `base`'s entries (and theirs, recursively) before the profile's own; a
missing base or a cycle is an error. Each base keeps the agents of its own profile. An entry
replaces an inherited one that names the same thing, so a child overrides a base instead of
repeating it. "The same thing" is: a skill or subagent by source and name, a plugin by marketplace
and name, an MCP server by name, a hook by event, matcher and command. The replacing entry brings
its own settings (agents, launch) and goes after the inherited ones. An inherited skill or subagent
entry that names several things loses only the ones the child names. Of two bases, the later one
wins.

### Applying

```sh
skillz profile save web          # snapshot what this project has installed (--force replaces)
skillz profile ls | show web | rm web
skillz use web --dry-run         # what installing `web` into this project would do
skillz use web                   # shows the plan, asks, then installs what is missing
skillz use web --exact           # also removes what `web` does not want (-y skips the question)
```

skillz works on one directory: the current one, or the one given with `-p PATH`. Profiles are
global (`~/.config/skillz/profiles`); nothing about a project is stored.

`profile save` keeps only what can be installed again: skills fetched by `npx skills` from a
source, plugins whose marketplace has one, subagents with a recorded source, hooks, and MCP
servers that need no secrets. The rest is listed as not saved, with the reason.

`use` is additive: it reads the project's inventory and installs only the entries (or the names
within an entry) that are not there yet, and never removes anything. `--exact` also removes every
installed item the profile does not want, after showing the plan; items with no known source
(hand-made skills and the like) are flagged because no profile can bring them back. An item
shared with a wanted entry, such as a skill for two agents of which the profile wants one, stays.

Which profile a project has loaded is never stored. A project matches a profile when nothing the
profile wants is missing from the inventory; the Project tab compares the directory with the
profile you choose.

## Running one profile in isolation

`skillz run web -- claude` loads the profile into config dirs of its own
(`~/.config/skillz/envs/web/{claude,codex}`, global scope), asks first unless `-y`, and starts the
command in the project with `CLAUDE_CONFIG_DIR` and `CODEX_HOME` pointing there. The command sees
that profile and nothing else, and switching is just running another profile: nothing is
uninstalled. The cost is a separate login and history per profile; the `npx skills` store under
`~/.agents` is still shared. skillz exits with the command's exit code.

## Isolating a config

When `CLAUDE_CONFIG_DIR` or `CODEX_HOME` is set, skillz reads and changes only that directory
for the agent's global config (skills, settings, plugins; for Claude also `.claude.json`) and
never `~/.claude` or `~/.codex`. The variable is also set on every command skillz runs
(`claude`, `codex`, `npx`), so native uninstalls act on the same directory. Project config
(`<project>/.claude`, `<project>/.codex`) is unaffected. Unset, the usual home dirs apply.

## Workspace

`skillz [PATH]` opens a workspace for one directory (default: the current one). It keeps nothing
per project: what is installed is read from the agents' own config each time. It needs a terminal
at least 40x10. Three tabs, switched with `tab` / `shift-tab` or `1` `2` `3`:

- **Project**: what the directory has installed, grouped as Skills, Plugins, MCP servers, Hooks and
  Subagents with `n of m` counts, followed in each group by library items it does not have yet
  (shown unchecked). A header shows the directory and how it compares with a profile you chose:
  matching, or `N missing` / `N extra`. Picked rows get their own marker and a `to install` /
  `to remove` note, and each group shows its count of picks.
- **Library** and **Profiles**: placeholders for now.

| Key | Does |
| --- | --- |
| `space` | pick the item under the cursor: a library item to install, an installed item to remove; again to unpick (on a group header: open or close it) |
| `enter` | apply all picks as one change, with one plan to review; with nothing picked, open or close the group |
| `esc` | clear the picks (with none, clear the filter) |
| `p` | choose the profile the header compares with |
| `a` | install what the chosen profile has that the project lacks |
| `←` `→`, `h` `l` | close / open a group (`←` on an item jumps to its group) |
| `↑↓` `j` `k`, `PgUp` `PgDn`, `^u` `^d`, `g` `G` | move |
| `/` | filter; keeps matching rows and their group, `esc` clears it |
| `?` | all keys for the screen |
| `q`, `^c` | quit |

Installing, removing and applying show a plan first: `=` kept, `-` removed, `+` installed, `~`
skipped, framed in red when it removes something. `enter` or `y` goes ahead, `d` turns on dry run,
`s` shows the raw steps, `esc` cancels. A progress screen follows, then a result with counts and
any failures: `c` copies them (through the terminal, so it works over ssh), `r` retries, `enter`
closes. A toast says what happened.

The browser (`list -i`, multi-group queries) and every picker (`uninstall`, `prune`, `install`)
share one list:

| Key | Does |
| --- | --- |
| `↑↓` `j` `k`, `PgUp` `PgDn`, `^u` `^d`, `g` `G` | move |
| `space`, `→` `l`, `←` `h` | expand/collapse (`←` on a collapsed row jumps to its parent) |
| `x`, `tab` | mark a group (all its items) or one item |
| `/` | filter; keeps matching rows and their parents |
| `i` | info: kind, scope, agents, source, installer, last use, contents, and the commands removal runs |
| `enter` | browser: Show / Remove… / Cancel for the marks (or the row under the cursor); picker: accept |
| `esc` | clear the filter, then quit |
| `q` | quit |

Environment: `SKILLZ_THEME=dark|light|mono` forces a theme. Otherwise `NO_COLOR` (non-empty) or
`TERM=dumb` selects mono, and `COLORFGBG` tells a light terminal from a dark one.
`SKILLZ_ASCII=1` draws plain ASCII symbols (also automatic for `TERM=linux`/`dumb` or a
non-UTF-8 locale). `SKILLZ_NO_MOUSE` (any value) turns mouse capture off.

## Safety

- **Complete but bounded removal.** Every deletion goes through one check: the path must sit
  strictly inside a root skillz owns (an agent's plugin cache, a skill dir, ...), built from plain
  names, and is skipped when it sits behind a symlinked directory. A symlink is removed, never its target,
  so a source checkout you linked in is untouched.
- **Native tools first.** Items an agent's own CLI installed are removed with that CLI
  (`claude plugin uninstall`, `codex mcp remove`, `npx skills remove`, ...); skillz then cleans
  what is left. A failing command aborts before any file is touched.
- **Additive by default.** `use` only installs; removal needs `--exact` and shows the plan first.
  A skill, hook or server is wanted when any one of its claims is, so what a wanted entry shares
  is never removed.
- **Atomic writes.** skillz's own files (profiles, source manifest) are written
  to a temp file and renamed. Edits to agent config files are staged beside the original with its
  permissions, follow symlinks (dotfiles stay links), are re-parsed before writing, and are
  written all or none; if a file changed since the plan was made, nothing is written. No backups
  are made.
- **Redaction.** Plans, logs and error messages hide URL credentials and query strings,
  `--token VALUE`, `NAME_SECRET=VALUE` and `Bearer` tokens.
- **Secrets are never saved.** MCP env vars and headers are read by name only, never value. A
  snapshot skips any MCP server that needs them, or that carries a credential in its args or URL.
- After removal skillz re-reads the inventory and reports anything still installed.

## What "remove" means

| Item | Removed |
| --- | --- |
| Skill (npx `skills` or loose) | every agent link to it, the store copy in `.agents/skills`, its lock entry |
| Hook group | every handler that runs the same tool, pruning empty groups/events |
| Subagent | the definition file and its `.skillz-sources.json` entry |
| MCP server | `claude mcp remove` / `codex mcp remove`, then its config entry |
| Claude plugin | `enabledPlugins` entry, install record, cache + data dirs once no scope uses it |
| Claude marketplace | its plugins, `known_marketplaces.json` entry, `extraKnownMarketplaces`, its clone (never a local directory source) |
| Codex plugin / marketplace | `config.toml` tables, hook trust state, plugin cache |

## Development

```sh
task dev                        # cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --summary-only   # coverage target > 75%
```

See [DESIGN.md](DESIGN.md) for architecture, on-disk formats and extension points.
Tests never assert on user-facing strings.
