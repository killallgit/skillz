# skillz

skillz manages the skills, plugins, marketplaces, hooks, MCP servers, and subagents that coding agents have installed. It reads each tool's native configuration as the truth and plans changes to it.

## Language

### Things managed

**Installed**:
An item a coding tool reports it has, with its identity, scope, agents, and the steps that would remove it.
_Avoid_: Entry, record

**Spec**:
An item someone wants installed, kept in a profile or the library.
_Avoid_: Desired item, requirement

**Harness**:
One coding tool (Claude, Codex) that skillz reads installed state from and plans changes for.
_Avoid_: Provider, backend, client

**Scope**:
Whether an item applies to the user's whole machine (global) or to one project.
_Avoid_: Level, context

**Plugin id**:
The `name@marketplace` identity of a plugin; a bare name has no marketplace.
_Avoid_: Plugin key, qualified name

### Loose skills

**Loose skill**:
A skill installed on its own rather than as part of a plugin.
_Avoid_: Standalone skill, bare skill

**Skill store**:
The shared directory that each agent's skill directory links into.
_Avoid_: Skill cache, skills folder

**Skill lock**:
The file recording where each loose skill came from and the content it was installed at. A missing lock means no skill is claimed; a corrupt lock is a reported failure, never an empty lock.
_Avoid_: Lockfile, manifest

**Lock ownership**:
The rule that decides whether a skill belongs to the skill lock: the store copy always does; a copy in an agent directory does only when the store holds no such skill.
_Avoid_: Provenance

### Hooks

**Hook format**:
One of the layouts a tool stores hooks in: nested JSON, flat JSON, TOML arrays, or inline TOML. Reading and removing a hook go through the same format.
_Avoid_: Hook syntax, hook schema

**Install record**:
The line in a Claude plugin registry that says a plugin was installed at a scope; removed together with the plugin so the tool no longer lists it.
_Avoid_: Registry entry, install entry

### Removal

**Removal plan**:
The ordered steps that uninstall one Installed item: native commands, file edits, then path deletions.
_Avoid_: Uninstall script, cleanup

**RemovalTarget**:
A path that is safe to delete: plain segments below a root skillz owns, with no symlinked ancestor. It is the only way to name a path for deletion.
_Avoid_: Delete path, removal path

**Uninstall flow**:
Choosing items, showing their removal plan, getting approval, executing it, and checking nothing was left behind.
_Avoid_: Remove command

### Changing state

**Action**:
One side effect in a plan: run a command, edit a file, remove a path, or copy a subagent.
_Avoid_: Step, operation

**Profile**:
A named set of Specs that can be applied, inheriting from other profiles.
_Avoid_: Preset, config

**Library**:
The catalog of every Spec skillz has seen, independent of any profile.
_Avoid_: Registry, catalog
