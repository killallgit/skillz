# skillz

List, install and completely remove skills, plugins, marketplaces and hooks for
Claude Code, Codex and Cursor — globally or per project — and keep named sets of them
(profiles) you can load into a project and swap.

```sh
task install                     # release build, symlinked to ~/.local/bin/skillz
# or: cargo install --path .
```

## Usage

```sh
skillz list                      # global + current project, groups collapsed
skillz list matt                 # matches in several groups: open the browser
skillz list ponytail             # expand what matches (a marketplace brings its group)
skillz list --all                # expand everything
skillz list --hooks .            # project hooks only
skillz list -g --skills -a claude
skillz list -i                   # browser: space expands, x marks, enter -> Show / Remove…
skillz list --json | jq '.[].name'  # machine-readable: items + removal plans

skillz rm tdd                    # fuzzy match -> pick -> confirm -> remove
skillz rm --hook bd -a cursor    # every hook that runs `bd`, for Cursor
skillz rm caveman@caveman -y     # non-interactive: needs one exact name match
skillz rm speckit --dry-run      # print the removal plan only

skillz reset                    # remove every skill, plugin, marketplace in scope (hooks with --hooks, MCP servers with --mcp)
skillz reset --keep caveman     # spare items by name, group or source
skillz reset --dry-run | --force
skillz prune                    # duplicates + unused 7+ days -> pick -> confirm -> remove
skillz prune --dry-run          # findings and their removal plan only
skillz prune --force            # remove everything found, no questions

skillz install mattpocock/skills --skill 'tdd,*commit' # glob match, confirm, npx skills add
skillz install mattpocock/skills --all -g
skillz install killallgit/killall-skills               # has a marketplace: pick Claude plugins
```

Scope: a `PATH` (or `-p PATH` (`--project`)) means that project only, `-g` means global only, neither
means both (project = cwd). A `list` PATH must start with `.`, `..` or `/` (`./sub`, not
`sub`); any other word is a query.

Pickers: `↑↓/jk` move, `space` toggle, `a` toggle all, `/` filter, `enter` accept, `esc` cancel.
Browser (`list -i`, multi-group queries): `space`/`→←` expand, `x`/`tab` mark a group or item,
`enter` opens Show / Remove… (same plan and `[y/N]` as `rm`), `/` filter, `q` quit.

## Profiles

`skillz` with no arguments opens the workspace: registered projects on the left, profiles in
the middle, details on the right. `Tab` switches pane, `Enter` applies the selected profile to
the selected project (the plan is shown and confirmed first), `u` unloads a project's profile,
`a` adds a project, `n` creates an empty profile, `s` saves what the project has installed as a
profile, `d` deletes, `q` quits. On a profile: `i` adds items the selected project has installed,
`t` types an entry (`skill SOURCE NAMES`, `plugin SOURCE MARKETPLACE NAME` or
`mcp NAME COMMAND|URL [ARGS]`, `hook EVENT COMMAND` or `subagent SOURCE NAMES`), `r` removes entries.

The same from the command line:

```sh
skillz profile save web          # snapshot this project's installs (-g: the global config)
skillz profile ls | show web | rm web
skillz use web --dry-run         # what swapping this project to `web` would remove and install
skillz use web                   # swap: only the difference is removed and installed
skillz use --none                # unload the current profile
skillz project add . | ls | rm . # projects the workspace shows
```

A profile is `~/.config/skillz/profiles/<name>.toml`:

```toml
agents = ["claude", "codex"]          # default for entries that name none

[[entry]]
kind = "skill"
source = "mattpocock/skills"
names = ["tdd", "grilling"]

[[entry]]
kind = "plugin"
source = "killallgit/killall-skills"  # what adds the marketplace
marketplace = "killallgit"
name = "killall-skills"
agents = ["claude"]
```

```toml
[[entry]]
kind = "mcp"
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
# or: url = "https://example.com/mcp", transport = "sse"   (default http)
```

```toml
[[entry]]
kind = "subagent"
source = "msitarzewski/agency-agents"    # a repo, or a directory relative to the project
names = ["frontend-developer", "ui-designer"]   # file names or the agents' own names
agents = ["claude"]                      # Claude copies .md files, Codex .toml files
```

Subagents are the definition files in `.claude/agents` / `~/.claude/agents` (Claude) and
`.codex/agents` / `~/.codex/agents` (Codex). skillz copies them from the source and remembers where
each came from (`.skillz-sources.json` beside them), so `list --subagents` groups them by source,
a snapshot can save them, and unloading deletes exactly those files. It never overwrites a file it
did not put there.

MCP servers are listed too (`skillz list --mcp`; read from Claude's `.claude.json` and
`.mcp.json`, and Codex's `config.toml`) and removed with their agent's own `mcp remove`. They are
spared by `reset` unless you pass `--mcp` (subagents: `--subagents`). A server that needs env vars or headers is not saved in
a profile (it would have to carry secrets); a snapshot lists it as skipped. Codex only installs MCP
servers globally, and has no SSE transport.

A profile can start from others: `extends = ["base"]` at the top loads `base`'s entries (and
theirs, recursively) before its own; a missing base or a cycle is an error. Each base keeps the
agents of its own profile.

skillz records what each entry added (`~/.config/skillz/applied.json`, per project and per
`CLAUDE_CONFIG_DIR`/`CODEX_HOME`), so swapping or unloading removes exactly that and never
what was installed by hand or was already there. Project plugins go to Claude's `local` scope,
not the settings file the team shares. Codex plugins can only be installed globally, so a
project profile reports them as skipped.

## Running one profile in isolation

`skillz run web -- claude` loads the profile into config dirs of its own
(`~/.config/skillz/envs/web/{claude,codex}`) and starts the command with `CLAUDE_CONFIG_DIR` and
`CODEX_HOME` pointing there. The command sees that profile and nothing else, and switching is just
running another profile: nothing is uninstalled. The cost is a separate login and history per
profile; the `npx skills` store under `~/.agents` is still shared.

## Isolating a config

When `CLAUDE_CONFIG_DIR` or `CODEX_HOME` is set, skillz reads and changes only that directory
for the agent's global config (skills, settings, plugins; for Claude also `.claude.json`) and
never `~/.claude` or `~/.codex`. The variable is also set on every command skillz runs
(`claude`, `codex`, `npx`), so native uninstalls act on the same directory. Project config
(`<project>/.claude`, `<project>/.codex`) is unaffected. Unset, the usual home dirs apply.

## What "remove" means

| Item | Removed |
| --- | --- |
| Skill (npx `skills` or loose) | every agent link to it, the store copy in `.agents/skills`, its lock entry |
| Hook group | every handler that runs the same tool, pruning empty groups/events |
| Claude plugin | `enabledPlugins` entry, install record, cache + data dirs once no scope uses it |
| Claude marketplace | its plugins, `known_marketplaces.json` entry, `extraKnownMarketplaces`, its clone (never a local directory source) |
| Codex plugin / marketplace | `config.toml` tables, hook trust state, plugin cache |

Every edited JSON/TOML file is re-parsed before it is written, and after removal skillz
re-discovers to confirm the items are gone. No backups are made.

## Development

```sh
task dev                        # cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo llvm-cov --summary-only   # coverage target > 75%
```

See [DESIGN.md](DESIGN.md) for architecture, on-disk formats and extension points.
Tests never assert on user-facing strings.
