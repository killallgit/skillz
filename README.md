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
| `list` (`ls`) | show what is installed as a tree, or browse it with `-i`; `--json` adds each item's location (`--check` also whether a newer version exists); `--available` lists the library items the project (`-g`: the user) lacks |
| `uninstall` (`rm`, `delete`) | fuzzy match, pick, confirm, remove completely; several queries go in one plan |
| `update` | update installed items with the tool that installed them (`-g`: the user's); `--check` lists the ones with a newer version (`--json` for scripts) |
| `reset` | remove every skill, plugin and marketplace in scope |
| `prune` | find duplicate and unused skills and plugins, remove the ones picked |
| `sync` | add everything installed, in the project and at user level, to the library (`-p PATH`, `-g` for user level only, `--dry-run`, `--json`); never removes anything from it |
| `install` (`add`) | `install OWNER/REPO` (or a git URL or path): offers each agent the plugins the repo has a marketplace for (Claude `.claude-plugin/`, Codex `.agents/plugins/`), installs skills for the rest with `npx skills`, and shows the plan before running it; `-g` installs for the user instead of the project |
| `library` (`lib`) | `list` (`ls`, `--json`, kind filters), `add "LINE"`, `remove` (`rm`) a label query or LINE, `install QUERY...` into the project or with `-g` for the user (each with `--dry-run`, `-y` where it asks) |
| `use` | add what a project lacks of a profile (`--exact` also removes the rest) |
| `run` | run a command against one profile in config dirs of its own |
| `profile` | `list` (`ls`, with how each compares with the project; `--json`), `status [NAME]` (present, missing, extra; `--json`), `show` (`export`), `import FILE`, `new`, `add NAME "LINE"` or `--from-library QUERY`, `remove NAME QUERY`, `save`, `delete` (`rm`) |
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
skillz list --json | jq '.[].name'  # items + removal plans; "package", "merged", "members"

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
skillz install mattpocock/skills --all -g                 # for the user, not the project
skillz update tdd -g                                     # update a user-level skill
skillz install killallgit/killall-skills                 # has a marketplace: pick plugins, review the plan
```

If a skill you ask for is already installed globally and not in the project, `install` says so
and asks before installing it into the project too (`-y` goes ahead).

Scope for `list`, `uninstall`, `reset` and `prune`: a `PATH` (or `-p PATH`) means that project only,
`-g` means global only, neither means both (project = cwd). `install` and `update` act on one
project, or with `-g` on the user's installs; `use`, `run` and `profile` always act on one project,
and the workspace has a tab for each. A project that is your home directory counts as global. A `list` PATH must
start with `.`, `..` or `/` (`./sub`, not `sub`); any other word is a query.

## From the workspace to the command line

Everything the workspace does is a command, and both run the same code (a plan reviewed first,
then the same install and removal steps):

| In the workspace | On the command line |
| --- | --- |
| Project / Global tabs: what is installed, the split, where each item lives | `list`, `list -g`, `list --json` (`location`) |
| Update markers | `list --json --check`, `update --check [--json]` |
| `space` picks, `enter` removes the picks as one plan | `uninstall QUERY...` (`-y`, `--dry-run`) |
| `r` remove / `u` update the highlighted item | `uninstall`, `update` |
| `A` add from a repo: type `owner/repo`, a git URL or a path | `install SOURCE` |
| `p` compare with a profile, the header's matches / N missing / N extra | `profile status [NAME] [--json]`, `profile list [--json]` |
| `a` apply a profile, `e` apply exactly | `use NAME`, `use NAME --exact` (`-y`, `--dry-run`) |
| Library: `a` add a typed line, `x` remove, `i` sync installed | `library add "skill o/r tdd"`, `library remove QUERY`, `sync` |
| Library: origin, link, version, installed in project / global | `library list [--json]` |
| Profiles: `n` new, `b` edit, `s` save, `x` delete | `profile new`, `profile add` / `profile remove`, `profile save`, `profile delete` |
| `/` filter, `space` fold, `?` help, `tab`, movement keys | on screen only: `list QUERY` and `list --all` do the same for the command line |

## Profiles

A profile is kept in skillz's one global store, `~/.config/skillz/store.json`, next to the library
(below). Names are letters, digits, `-`, `_` and `.` and do not start with `-` or `.`. The examples
below are the TOML that `skillz profile show` prints.

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
| plugin | `claude plugin marketplace add` + `plugin install`; project scope uses Claude's `local` scope, never the file the team shares | `codex plugin marketplace add` + `plugin add`, global only |
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

Which profile a project has loaded is never stored. A project matches a profile when nothing the
profile wants is missing from the inventory; the Project tab compares the directory with the
profile you choose.

## The store and the library

`~/.config/skillz/store.json` is the only file skillz keeps for itself (apart from the scratch
config dirs `run` makes). It holds `version`, `items` (the library) and `profiles`. The library is
every skill, plugin, MCP server, hook and subagent you have installed or typed in, so that
anything installed once can be installed again from the Project or Global tab. Each item records its
provenance: the `link` it was fetched from, the `engine` that installed it, the `version` (a
plugin version, or the content hash `npx skills` locked) and when it was `recorded_at`. Every
successful `install`, `use` and workspace change adds what it installed.

The library is a master record, so it also learns what you installed some other way. When the
workspace opens, a background scan reads everything installed in the project and at user level,
for every agent (skills, plugins, MCP servers, hooks, subagents), and adds what the library does
not have yet, marked as *detected* in its provenance. The workspace is shown at once; when the scan
ends the Library tab fills and a toast says how many were new (and how many MCP servers were left
out for needing secrets). The same scan runs as `skillz sync`, after every install, update or
change, and on `i` in the Library tab. It only ever adds: a second run adds nothing, labels you
gave are kept, and items that are no longer installed stay. The only way out of the library is its
remove key (`x`) or removing it by hand in `store.json`. `SKILLZ_NO_SYNC=1` turns the startup scan
off. A hand-made skill or subagent is recorded by the directory that holds it (engine `local`,
marked detected), so it is reinstalled with `npx skills add <that directory> --skill <name>`.
Items needing secrets, and items with nothing to reinstall them from, are not recorded.

**Moving a folder.** skillz keeps no registry of projects, so moving a project is fine, but a
hand-made skill or subagent is recorded by its absolute directory. When that directory is gone the
item is *stale*: the Library tab tags it `! missing source`, `library list` marks it and
`--json` has `"stale": true`, and `profile status` lists profile entries in that state as
`unavailable` (apart from `missing`). The next sync (startup scan or `skillz sync`) re-points a
stale item at the new directory in place when exactly one live local source holds a skill or
subagent of the same name (it keeps your label, refreshes the provenance, and follows the profile
entries that named the old path); the toast says `moved: N` and `sync --json` has `repointed`. If
several places match it stays stale (`ambiguous`), and if the library already has the live item the
stale duplicate is dropped. Repository and URL sources are never touched. Installing or applying
an entry whose directory is missing fails that entry, naming it and the path, before `npx` runs;
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

`skillz [PATH]` opens a workspace for one directory (default: the current one). It keeps nothing
per project: what is installed is read from the agents' own config each time. It needs a terminal
at least 40x10. Four tabs, switched with `tab` / `shift-tab` or `1` `2` `3` `4`:

- **Project** and **Global**: the same tab over two scopes. Project is what this directory has
  installed; Global is what you have installed at user level, outside any project. Each lists
  only what is installed there, in sections by kind: `plugins`, `skills`, `agents`,
  `MCP servers` and `hooks` (a section with nothing in it is left out). Every row is a whole
  unit you can remove or update: a plugin is one row that says what it provides
  (`3 skills, 2 hooks`), and what it provides is not listed again among the skills and hooks;
  removing the plugin removes them. Skills hang under the package they came from, as
  `npx skills list` does: an `owner/repo` or `local files`. `hooks` are only the hooks no plugin
  declares. Every row shows its agents and where it lives on disk (the skill's directory, or the
  config file). The panel title is the directory, or `User-level, all projects`; on Project a
  line above it shows how the directory compares with a profile you chose: matching, or
  `N missing` / `N extra`. Picked rows get their own marker and a `to remove` note, and each
  section or package shows its count of picks. `space`, `r` and `u` on a package row act on
  everything in it (the plan shows what, and nothing happens until you confirm it); on an item
  row they act on that item; on a section `space` folds it, `r` marks every item in it for removal and `u` does nothing. A thing installed for
  several agents (a subagent for Claude and Codex, say) is one row naming its agents, `claude,
  codex`, or `claude, codex +2` when there are many; removing it removes every copy. A filter
  keeps the section structure and hides rows with no match. Removals and updates on Global act on
  the user's installs (`npx skills -g`, `claude ... --scope user`, the Codex user config).
  Profiles apply to a project only, so Global has no `p` or `a`. Adding is `A`, or the
  Library tab and profiles.
  In the background the tabs check whether installed Claude marketplaces have a newer version
  (a read-only `ls-remote`) and shows those in bold with a `*` after the name. Other kinds have
  no reliable check and are never marked. `SKILLZ_NO_UPDATE_CHECK=1` turns the check off;
  `skillz update --check` runs it from the command line.
- **Library**: the items skillz knows about, grouped by package like the other tabs, with where
  each came from. The line under the list
  starts with how it got there (*detected*, *added by skillz* or *added by hand*), then shows the
  link, how it was installed (`npx skills`, `claude plugin`, `codex plugin`, `manual`) and the
  version or hash recorded. A `project` or `global` tag after a name says where the item is
  installed right now; no tag means it is only in the library.
- **Profiles**: your saved combinations, each with how it compares with the project. Apply one
  (adds what is missing) or apply it exactly (also removes the rest, after a preview), save what
  is installed as a new one, or delete one. It is one list, each profile with its entries under
  it. `n` opens a single modal: type the name, `tab` to the library items, `space` to check
  them, and `s` (or `enter` in the name) saves. `b` edits the profile under the cursor in the
  same modal.

| Key | Does |
| --- | --- |
| `space` | pick the item under the cursor for removal; again to unpick (on a package: everything in it; on a section: open or close it) |
| `enter` | remove all picks as one change, with one plan to review; with nothing picked, open or close the section |
| `r` | remove the item under the cursor from the tab's scope at once (after the plan); on a package row, removes everything in it; on a section, marks every item in it for removal. When the items are installed for more than one agent, the plan shows an Agents list above what will be removed; every agent starts checked and `space` unchecks one (`↑↓` moves) before `enter` applies |
| `esc` | clear the picks (with none, clear the filter) |
| `u` | update the installed item under the cursor, or every installed item of the package, after a plan to review; kinds with nothing to update (hooks, MCP servers, hand-made skills) are ignored |
| `p` | choose the profile the header compares with (Project only) |
| `a` | install what the chosen profile has that the project lacks (Project only) |
| `←` `→`, `h` `l` | close / open a section (`←` on an item jumps to its section) |
| `↑↓` `j` `k`, `PgUp` `PgDn`, `^u` `^d`, `g` `G` | move |
| `/` | filter; keeps matching rows and their section, `esc` clears it |
| `?` | all keys for the screen |
| `q`, `^c` | quit |

Installing, removing, updating and applying show a plan first: `=` kept, `-` removed, `+`
installed, `^` updated, `~` skipped, framed in red when it removes something. `enter` or `y` goes ahead, `d` turns on dry run,
`s` shows the raw steps, `esc` cancels. A progress screen follows, then a result with counts and
any failures: `c` copies them (through the terminal, so it works over ssh), `r` retries, `enter`
closes. A toast says what happened.

While a change runs, a modal over the tab lists every item it will change, each with a spinner
(`⠋`, or `|/-\` in ASCII mode) while it works, then a tick or a cross when it finishes; the
affected rows on the Project and Global tabs show the same marks until the next key. The result screen lists what was done (`Installed X`, `Removed Y`, `Updated Z`) and
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
| Hook group (`(unknown) hooks`; a plugin's own hooks go with the plugin) | every handler that runs the same tool, pruning empty groups/events |
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
