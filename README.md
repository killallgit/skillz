# skillz

List, install and completely remove skills, plugins, marketplaces, hooks, MCP servers and
subagents for Claude Code and Codex, globally or per project, and keep named sets of them
(profiles) that you can load into a project and swap.

Download the archive for your platform from the
[latest release](https://github.com/killallgit/skillz/releases/latest) (macOS and Linux, Intel
and ARM), check it against the `.sha256` beside it, and put `skillz` on your `PATH`. Or build it:

```sh
task install                     # release build, symlinked to ~/.local/bin/skillz
# or: cargo install --path .
```

Releases are cut by pushing a tag that matches the version in `Cargo.toml`
(`git tag v0.1.0 && git push origin v0.1.0`); CI builds the four binaries and publishes them.

`skillz` with no arguments opens the workspace (see [Workspace](#workspace)); everything it does
is also a command.

## Commands

| Command | Does |
| --- | --- |
| `list` (`ls`) | in a terminal, a plain `list` (or `list -g`) opens the plugins list (see Workspace); otherwise (a query, `--all`, a kind filter, no terminal) it shows what is installed as a tree, or browse it with `-i`; `--json` never opens the list and prints the items for scripts and agents, adding each item's location (`--check` also whether a newer version exists); `--available` lists the library items the project (`-g`: the user) lacks |
| `uninstall` (`rm`, `delete`) | fuzzy match, pick, confirm, remove completely; several queries go in one plan |
| `update` | update installed items with the tool that installed them (`-g`: the user's); `--check` lists the ones with a newer version (`--json` for scripts) |
| `reset` | remove every skill, plugin and marketplace in scope |
| `prune` | find duplicate and unused skills and plugins, remove the ones picked |
| `doctor` | check the skills on disk, read-only: names that collide (per tool, listing every copy), skill files nested or loose where a tool will not look (or looks too hard), files that break the format, and broken links; `-g` for the user's skills only, `--json` for scripts |
| `sync` | add everything installed, in the project and at user level, to the library (`-p PATH`, `-g` for user level only, `--dry-run`, `--json`); never removes anything from it |
| `install` (`add`) | `install OWNER/REPO` (or a git URL or path): offers each agent the plugins the repo has a marketplace for (Claude `.claude-plugin/`, Codex `.agents/plugins/`), installs skills for the rest with `npx skills`, and shows the plan before running it; `-g` installs for the user instead of the project |
| `library` (`lib`) | `list` (`ls`, kind filters; groups the items under a `── package ──` rule per package or folder and writes your home directory as `~`, while `--json` keeps full paths), `add "LINE"`, `remove` (`rm`) a label query or LINE, `install QUERY...` into the project or with `-g` for the user (each with `--dry-run`, `-y` where it asks) |
| `use` | add what a project lacks of a profile (`--exact` also removes the rest) |
| `run` | run a command against one profile in config dirs of its own |
| `profile` | `list` (`ls`, with how each compares with the project; `--json`), `status [NAME]` (present, missing, extra; `--json`), `show` (`export`), `import FILE`, `new`, `add NAME "LINE"` or `--from-library QUERY`, `remove NAME QUERY`, `save`, `delete` (`rm`) |

Kind filters on `list`, `uninstall`, `reset` and `prune`: `--skills --hooks --plugins
--marketplaces --mcp --subagents` (singular spellings work too; none given means all kinds).
`-a claude|codex` filters by agent (repeatable), `-g` means global only. Run
`skillz <command> --help` for every flag.

```sh
skillz list                      # global + current project, groups collapsed
skillz list matt                 # matches in several groups: open the browser
skillz list ponytail             # expand what matches (a marketplace brings its group)
skillz list --all                # expand everything
skillz list --hooks .            # project hooks only
skillz list -g --skills -a claude
skillz list -i                   # browser: expand, mark, enter -> Show / Remove…
skillz list --json | jq '.[].name'  # items + removal plans; "package", "merged", "members"

skillz rm tdd                    # fuzzy match -> pick -> confirm -> remove
skillz rm --hook bd -a codex     # every hook that runs `bd`, for Codex
skillz rm caveman@caveman -y     # non-interactive: needs exactly one exact name match
skillz rm speckit --dry-run      # print the removal plan only

skillz reset                     # skills, plugins, marketplaces; --hooks, --mcp, --subagents add those kinds
skillz reset --keep caveman      # spare items by name, group or source
skillz reset --dry-run | --force
skillz prune                     # duplicates + unused 7+ days -> pick -> confirm -> remove
skillz prune --dry-run | --force

skillz install mattpocock/skills --skill tdd             # installs the plugin that supplies tdd
skillz install mattpocock/skills --all
skillz install mattpocock/skills --all -g                 # for the user, not the project
skillz update tdd -g                                     # update a user-level skill
skillz install killallgit/killall-skills                 # has a marketplace: pick plugins, review the plan
```

Source installs prefer each tool's native plugin installer when a marketplace is available.
`--all` installs every offered plugin; `--skill` selects the plugins that supply the matching
skills, and installs everything such a plugin ships. The plan names each plugin with how many
skills it brings, and for `--all` lists the skills of the source that no plugin supplies, which
are not installed (name them with `--skill`). Sources without a matching plugin use `npx
skills`. A malformed marketplace or plugin manifest in the source never fails the install, its
skills install loose instead. `library install`, `use`, `run` and the list install a skill entry
the same way: the plan fetches its source, and each agent whose marketplace supplies every named
skill installs the owning plugin; the others, and all of them when the fetch fails, use `npx
skills`.

A Codex project install writes the marketplace and plugin into `.codex/config.toml`, which a team
usually commits, so skillz refuses a source with a user, password or token in its URL (use ssh or
a credential helper; this includes a bare `https://user@` account), and a local source is written
as this machine's path. It also refuses a marketplace name you already registered for yourself
from another source, or pinned to another revision or a sparse checkout: Codex keeps one
downloaded copy per name for every project, and the project install would replace it. The same
repo under another spelling (`o/r`, `https://github.com/o/r.git`, `git@github.com:o/r.git`) is
the same source. A refusal skips that agent and says why in the plan; the other agents still
install, and an install every agent refuses fails with the reasons. A config that does not parse,
or has the wrong shape, is refused before anything is downloaded.

If a skill you ask for is already installed globally and not in the project, `install` says so
and asks before installing it into the project too (`-y` goes ahead).

Scope for `list`, `uninstall`, `reset` and `prune`: a `PATH` (or `-p PATH`) means that project only,
`-g` means global only, neither means both (project = cwd). `install`, `update` and `use` act on one
project, or with `-g` on the user's installs; `run` and `profile` always act on one project. A project that is your home directory counts as global. A `list` PATH must
start with `.`, `..` or `/` (`./sub`, not `sub`); any other word is a query.

## From the list to the command line

Everything the list does is a command, and both run the same code (a plan reviewed first, then
the same install and removal steps). What the list does not do is a command too:

| In the list | On the command line |
| --- | --- |
| What is installed, the split, where each item lives | `list`, `list -g`, `list --json` (`location`) |
| `space` selects, `enter` submits installs and removals as one plan | `install SOURCE`, `uninstall QUERY...` (`-y`, `--dry-run`) |
| `i` info: provenance and locations | `list --json` (`location`), `library list --json` |
| Updating | `update [QUERY] [-g]`, `update --check [--json]` |
| Applying and comparing a profile | `use NAME [--exact] [-g]`, `profile status [NAME] [--json]`, `profile list` |
| Saving the project as a profile, editing profiles | `profile save`, `profile new`, `profile add` / `profile remove`, `profile delete` |
| Clearing the project | `reset` removes skills, plugins and marketplaces; add `--hooks`, `--mcp`, `--subagents` for those kinds |
| Keeping only some entries of an applied profile | no command yet (bead `skillz-obs`) |
| The library | `library add "skill o/r tdd"`, `library remove QUERY`, `library install QUERY [-g]` (each entry installs for the agents it names; there is no `-a`), `sync` |
| `/` filter, `?` help, movement keys | on screen only: `list QUERY` and `list --all` do the same for the command line |

## Profiles

A profile is kept in skillz's one global store, `~/.config/skillz/store.json`, next to the library
(below). Names are letters, digits, `-`, `_` and `.` and do not start with `-` or `.`. The examples
below are the TOML that `skillz profile show` prints.

```toml
description = "frontend work"         # optional
extends = ["base"]                    # optional, see below
agents = ["claude", "codex"]          # default for entries that name none (this is the default)

[[entry]]
kind = "skill"                        # the owning plugin, else `npx skills`
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
`command` or `url`. Codex takes what its CLI and config can (see below); anything an agent cannot take is shown as skipped in the plan, not as an error.

| Kind | Claude | Codex |
| --- | --- | --- |
| skill | the owning plugin when the source's marketplace supplies every name, else `npx skills add` with the other agents | same |
| plugin | `claude plugin marketplace add` + `plugin install`; project scope uses Claude's `local` scope, never the file the team shares | Global: `codex plugin marketplace add` + `plugin add`. Project: marketplace and plugin settings in `.codex/config.toml`; Codex materializes them on a marketplace refresh in a trusted project |
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
skillz use web --dry-run         # what installing `web` would do; installs nothing, but still fetches skill sources to plan
skillz use web                   # shows the plan, asks, then installs what is missing
skillz use web --exact           # also removes what `web` does not want (-y skips the question)
skillz use web -g                # the same at user level: plugins, skills and the rest for every project
```

skillz works on one directory: the current one, or the one given with `-p PATH`. Profiles are
global (the store); nothing about a project is stored. A project's state is its own tool config.

`profile save` keeps only what can be installed again: skills fetched by `npx skills` from a
source, plugins whose marketplace has one, subagents with a recorded source, hooks, and MCP
servers that need no secrets. A hand-made skill or subagent is kept by the local directory that
holds it (a skill only when `npx skills` can see it: `name` and `description` set, and inside
your home, the project or the agents' config dirs), so such a profile applies on this machine
only. The rest is listed as not saved, with the reason.

`use` is additive: it reads the project's inventory and installs only the entries (or the names
within an entry) that are not there yet, and never removes anything. `--exact` also removes every
installed item the profile does not want, after showing the plan; items with no known source
(plugins without a marketplace, skills `npx skills` cannot see) are flagged because no profile
can bring them back. An item
shared with a wanted entry, such as a skill for two agents of which the profile wants one, stays.

`use` acts on one level only. In a project it installs into that project (Claude plugins at
`local` scope, so only that project has them) and counts only what the project holds; with `-g`
it installs at user level (Claude `--scope user`, `npx skills -g`, the Codex user config) and
counts only the user's own installs. A profile applied at user level shows up in the project list
below the divider, read-only, and does not make the project match it. Codex plugins can only be
installed at user level, so a project apply skips them.

Which profile a project has loaded is never stored. A project matches a profile when nothing the
profile wants is missing from the inventory; `profile status` compares the directory with the
profile you choose.

## The store and the library

`~/.config/skillz/store.json` is the only file skillz keeps for itself (apart from the scratch
config dirs `run` makes). It holds `version`, `items` (the library) and `profiles`. The library is
every skill, plugin, MCP server, hook and subagent you have installed or typed in, so that
anything installed once can be installed again with `library install`. Each item records its
provenance: the `link` it was fetched from, the `engine` that installed it, the `version` (a
plugin version, or the content hash `npx skills` locked) and when it was `recorded_at`. Every
successful `install`, `use` and workspace change adds what it installed.

The library is a master record, so it also learns what you installed some other way. When the
workspace opens, a background scan reads everything installed in the project and at user level,
for every agent (skills, plugins, MCP servers, hooks, subagents), and adds what the library does
not have yet, marked as *detected* in its provenance. The workspace is shown at once; when the scan
ends the list gains them and a toast says how many were new (and how many MCP servers were left
out for needing secrets). The same scan runs as `skillz sync`, after every install, update or
change. It only ever adds: a second run adds nothing, labels you
gave are kept, and items that are no longer installed stay. The only way out of the library is
`skillz library remove` or removing it by hand in `store.json`. `SKILLZ_NO_SYNC=1` turns the startup scan
off. A hand-made skill or subagent is recorded by the directory that holds it (engine `local`,
marked detected), so it is reinstalled from that directory: through the owning plugin when the
directory's marketplace supplies it, else with `npx skills add <that directory> --skill <name>`.
Items needing secrets, and items with nothing to reinstall them from, are not recorded.

**Moving a folder.** skillz keeps no registry of projects, so moving a project is fine, but a
hand-made skill or subagent is recorded by its absolute directory. When that directory is gone the
item is *stale*: `library list` marks it as missing its source and
`--json` has `"stale": true`, and `profile status` lists profile entries in that state as
`unavailable` (apart from `missing`). The next sync (startup scan or `skillz sync`) re-points a
stale item at the new directory in place when exactly one live local source holds a skill or
subagent of the same name (it keeps your label, refreshes the provenance, and follows the profile
entries that named the old path); the toast says `moved: N` and `sync --json` has `repointed`. If
several places match it stays stale (`ambiguous`), and if the library already has the live item the
stale duplicate is dropped. Repository and URL sources are never touched. Installing or applying
an entry whose directory is missing, or a skill entry whose directory exists but no longer holds
that skill, fails that entry, naming it and the path, before `npx` runs (only a missing directory
is followed by `skillz sync`);
the other entries still install. `skillz run` environments are keyed by profile only, so a moved
project reuses its own.
Installing a skill again fetches it from its recorded
link when there is one.

The version is recorded and shown, not pinned: `npx skills add`, `claude plugin install` and
`codex plugin add` have no option to install a given revision, so a reinstall gets the latest.

The CLI and the workspace may write at the same time; every change is read, modified and written
back under a lock. A store that cannot be parsed is reported and left untouched.

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

`skillz [PATH]` opens the plugins list for one directory (default: the current one). It keeps
nothing per project: what is installed is read from the agents' own config each time. It needs a
terminal at least 40x10. `skillz list` in a terminal (with no query, flag or kind filter) opens
the same list instead of printing, and `skillz list -g` (`--global`) opens it for the user's
installs alone, with no project rows and no global section; flipping a package there installs or
removes it at user level.

The list has a line per package (a marketplace or an `owner/repo`), from the library and from
what the project has, by name, with no column header. A checkbox says whether the project has
it, and an installed one names its agents, `[claude, codex]`. Hooks, MCP servers and hand-made
files are not in it yet.

- `space` selects or deselects the package under the cursor: its line turns bold and its box
  shows what it will become, and nothing happens yet. `esc` drops the selection.
- `enter` submits everything selected as one change. The plan lists `installing` and `removing`
  groups to confirm (an install is for every agent; an entry that names its own agents keeps to
  them), each skill or plugin with its own box, all checked: `↑` `↓` move and `space` unchecks
  one you want to leave out.
- `i` opens an info modal for the package under the cursor: where it comes from, how the library
  came to know it (added by hand, found by a scan or installed by skillz, when, what it is
  fetched from, with which tool and which version), what it offers and where each installed item
  lives on disk.
- `/` filters.

There are no tabs: updating, removing by query, profiles and library editing are commands (see
the table above).

| Key | Does |
| --- | --- |
| `space` | select the package under the cursor, or unselect it |
| `enter` | submit everything selected as one change, after a plan to review |
| `esc` | drop the selection (with none, clear the filter) |
| `i` | info: where the package comes from and where each installed item lives |
| `/` | filter; keeps matching rows, `esc` clears it |
| `↑↓` `j` `k`, `PgUp` `PgDn`, `^u` `^d`, `g` `G` | move |
| `?` | all keys for the screen |
| `q`, `^c` | quit |

Project profile applies declare Claude marketplaces and install plugins in the project's
`.claude/settings.local.json`. Claude keeps downloaded marketplaces and plugin caches under
its user configuration directory; that shared storage does not make them global installs.
Marketplace scope comes from `extraKnownMarketplaces` in settings, not from the shared
`known_marketplaces.json` download registry. Adding a plugin to the project declares its
marketplace in the project's `.claude/settings.json`, the same scope the plugin is enabled in.

Installing, removing, updating and applying show a plan first: `=` kept, `-` removed, `+`
installed, `^` updated, `~` skipped, framed in red when it removes something. It ends in two
buttons, `[ Cancel ]` and `[ Delete ]` (`[ Apply ]` when nothing is removed), with no help text:
`←` `→` or `tab` change the button and `enter` answers it, `y` goes ahead and `n` or `esc` cancels.
`d` turns on dry run (the button then reads `Dry run`) and `s` shows the raw steps. A removal that
spans both agents lists them as checkboxes first; `space` unchecks one. Each plan line is the
item's name, its source when that adds something, and its kind. A plain confirm dialog is the
question with `[ No ]` and `[ Yes ]`, the cancelling one first. A progress screen follows, then a short result: a line for each item that changed and an
`[ OK ]` button, with no help text. If something failed, its reasons are listed and `[ Copy ]`
(through the terminal, so it works over ssh) and `[ Retry ]` come before OK; `←` `→` or `tab`
change the button, `enter` answers it, and `c` and `r` still work directly. A toast says what
happened.

While a change runs, a modal over the list lists every item it will change, each with a spinner
(`⠋`, or `|/-\` in ASCII mode) while it works, then a tick or a cross when it finishes; the
affected rows in the list show the same marks until the next key. The result screen lists what was done (`Installed X`, `Removed Y`, `Updated Z`) and
every failure with its reason.

The browser (`list -i`) and every picker (`uninstall`, `prune`, `install`)
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
| Hook group (`(unknown) hooks`; a plugin's own hooks go with the plugin) | every handler that runs the same tool, in every agent that has one (one row, `bd  claude, codex`), pruning empty groups/events |
| Subagent | the definition file and its `.skillz-sources.json` entry |
| MCP server | `claude mcp remove` / `codex mcp remove`, then its config entry |
| Claude plugin | `enabledPlugins` entry, install record, cache + data dirs once no scope uses it |
| Claude marketplace | declarations and plugins in the selected scope; shared installs and metadata stay. An unused registry entry and owned clone are removed (never a local directory source) |
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
