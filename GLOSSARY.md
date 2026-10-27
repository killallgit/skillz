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

### Plugins list

**Package**:
What the plugins list shows a line for: a marketplace, an `owner/repo`, or the installed items and library entries that share one source. Selecting it and submitting installs or removes all of it as one change.
_Avoid_: Plugin group, bundle

**Plugins list**:
The one interactive screen: a checkbox per package, `space` to select, `enter` to submit one plan. It has no tabs.
_Avoid_: Workspace tabs, Project tab

### Loose skills

**Loose skill**:
A skill installed on its own rather than as part of a plugin.
_Avoid_: Standalone skill, bare skill

**Skill store**:
The shared directory that each agent's skill directory links into.
_Avoid_: Skill cache, skills folder

**Skill lock**:
The file recording where each loose skill came from and the content it was installed at.
_Avoid_: Lockfile, manifest

**Lock ownership**:
The rule that decides whether a skill belongs to the skill lock: the store copy is eligible; a copy in an agent directory is eligible only when the store holds no such skill.
_Avoid_: Provenance

### Hooks

**Hook format**:
One of the layouts a tool stores hooks in. Reading and removing a hook go through the same format.
_Avoid_: Hook syntax, hook schema

**Install record**:
The line in a tool's plugin registry that says a plugin was installed at a scope; removed together with the plugin so the tool no longer lists it.
_Avoid_: Registry entry, install entry

### Removal

**Removal plan**:
The steps that uninstall one Installed item.
_Avoid_: Uninstall script, cleanup

**RemovalTarget**:
A path that is safe to delete because it lies inside a root skillz owns. It is the only way to name a path for deletion.
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
