# skillz

List, install and completely remove skills, plugins, marketplaces and hooks for
Claude Code, Codex and Cursor — globally or per project.

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

skillz reset                    # remove every skill, plugin, marketplace in scope (hooks with --hooks)
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
