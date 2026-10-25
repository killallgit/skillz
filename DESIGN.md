# skillz design

skillz manages skills, plugins, marketplaces, hooks, MCP servers, and subagents for
coding agents. Native tool configuration is the source of installed state. The
one skillz store holds the library and named profiles.

## Read, plan, execute

Discovery reads configuration through each `AgentAdapter`, plus shared skill,
hook, and subagent readers. Missing files and directories are empty; other
failures become inventory warnings. Display flows can show a partial inventory.
Flows that change or record installed state require a complete inventory.

An `Installed` contains identity, scope, agents, source, typed `Details`, and
its removal actions. Details retain paths, install names, plugin skills, and MCP
launch information. Display children are presentation only. Hooks retain typed
handlers. Merging logical items preserves their per-agent members and removal
plans.

Profiles contain typed `Spec` values. `profile::Store` reads the store once per
resolution, then resolves inheritance in memory with cycle detection and cached
results. Child entries replace inherited entries with matching identities;
partially overlapping skill/subagent entries retain the other names. Agent lists
are resolved before comparison with installed claims.

`apply::compare` reports present entries, missing entries, and extras. Planning
uses adapter provisioning to turn missing entries into actions, and reuses
removal plans for extras only in exact mode. Snapshotting converts typed installed
facts into reproducible entries; it reports missing sources, invalid definitions,
and secret-bearing MCP servers as skipped.

`Applier::run` is the execution path for profile applies, selected changes, and
updates. It removes unwanted items first, then installs and updates item by item.
Independent items continue after a failure. Successful shared install steps run
once. A missing local source fails before launching its install command. Progress
reports item start/end events and action steps.

`install` turns a repo or path into one plan: each agent that offers the source plugins (`AgentAdapter::plugin_offer`)
asks which to install and returns its steps (`plugin_steps`); the remaining agents install the source's skills through
`npx skills`. The steps are shown as one plan and run only once approved. The CLI and the workspace
share `Session::install_source`, which also records what arrived in the library.

`cli::session::change` owns plan review, execution, inventory recording, result
presentation, and retry. Plain commands use its `Console` prompter; the workspace
uses the TUI prompter. Cancellation remains distinct from successful preparation,
so an isolated command launches only after its profile is ready.

## Effects and safety

- `Env` carries home, project, and redirected config roots. `CommandRunner` owns
  external processes; `Prompter` owns interaction. Discovery, source readers, and
  stores perform explicit filesystem reads.
- `Action` represents commands, file edits, owned path removals, and subagent
  copying. `apply_edit` transforms text without filesystem effects.
- The executor deduplicates actions, runs commands, stages edits, commits files,
  then removes paths. External commands and deletions cannot be rolled back.
- Staging writes temporary files beside their resolved targets, retains existing
  permissions, and checks that originals have not changed. Commit failures roll
  back earlier replacements where their committed content is still present.
- Subagent copying resolves and validates every destination before staging its
  definitions and ownership manifest together. Foreign files and duplicate
  destination names are refused.
- `RemovalTarget` accepts plain path segments below an owned root. Execution
  checks ownership again and avoids deletion through symlinked ancestors.
- The global store uses an advisory sidecar lock and atomic replacement. Invalid
  JSON or unsupported versions produce `Corrupt` and are never overwritten.
- Displayed commands and failures redact credentials. MCP secrets retain names
  only; snapshots exclude servers that need credentials.
- Library sync adds discovered items and follows unambiguous moved local sources
  in both library and profile entries. Explicit library removal remains separate
  from uninstalling native configuration.

## Module ownership

| Module | Responsibility |
| --- | --- |
| `model`, `env` | Installed facts, identities, scopes, and filesystem roots |
| `agents` | Per-agent discovery, provisioning, updates, usage, and native command knowledge |
| `harness::npx` (loose skills, `npx skills` commands and lock file), `hooks`, `subagents`, `mcp` | Shared formats and typed installed facts |
| `install`, `harness::clone`, `subagent_store` | Source resolution, selection, and installation |
| `harness::action`, `harness::read_optional` | Pure edits, shared staged writes, owned deletion, and filesystem policy |
| `profile`, `apply`, `snapshot` | Desired entries, inheritance, comparison, plans, and execution |
| `store`, `library`, `sync`, `stale` | Persistent catalog/profiles, provenance, and moved sources |
| `merge`, `package` | Logical copies and source-package grouping |
| `usage`, `cost`, `prune`, `update` | Shared reporting and maintenance decisions |
| `review`, `prompt` | Typed plans, verdicts, outcomes, progress, and interaction interfaces |
| `cli` | Command parsing, scope selection, sessions, and real dependency wiring |
| `tree`, `tui` | Plain tables, reusable list/outline widgets, and interactive presentation |

## Interaction

Project and Global share `InstalledTab`. Rows resolve into actionable items once,
so installing, removing, and updating a package do not depend on expansion state.
Library and profile row values carry the facts their presentation needs.

`Outline` owns navigation and filtering. Screens resolve actions through
`keymap::BINDINGS`; the same bindings generate help and hints. Plan and result
screens share a text modal that accounts for wrapped visual lines and viewport
height. `App` drives one screen; local popup enums describe dialogs within tabs.
Terminal ownership and restoration remain in the shell and its hold guards.

Plain `list QUERY` prints matching items. Browsing is explicit with `list -i`.
Background checks examine each installed id once per workspace session, then
recheck updated items and newly discovered ids. The README owns key tables and
command examples.

## Extension and tests

An agent implements `AgentAdapter` and joins `agents::registry()`. Add its closed
`Agent` enum variant for clap/serde and its external skills id. Keep native paths,
formats, commands, and update checking in that adapter, except loose skills: the
`harness::npx` module owns every loose skill and the directory each agent reads them from
(`npx::skill_dirs`), so a new agent also adds an arm there. Shared flows receive the
adapter list, and `tests/conformance.rs` verifies that injected adapters participate.

Tests exercise established interfaces using temporary homes, fake runners, and
scripted prompters. Keep behavioral coverage for ownership, partial failure,
cancellation, scope isolation, and typed plans. Delete tests for removed machinery
and duplicate presentation assertions when stronger behavior tests cover them.
