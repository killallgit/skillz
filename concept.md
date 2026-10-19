Create a rust cli using clap, for managing skills/plugins/hooks for coding agents... focus only on claude/codex/cursor to start. Make the system easy to add onto in the future by following well estabilished programming principles. use dependency injection and functional style. Keep the test coverage > 75% and absoltuely do not assert on strings that would be output like error strings or other user messages that change often.

The TUI elements need to be very basic, a list where you can mulit-select, a collapsable list/tree and a "/" find type input.

Here's some ideas for commands.

```

skillz list # list all skills, plugins, hooks, and display them like a file tree so that the main plugin/marketplace/skill is the group that contains the children depending on the "augmentation" (skill, plugin, marketplace, hooks)

skillz list --hooks . # <- . is the path, -g,--global obviously global...

skillz list --skills --agent claude|codex|cursor, et.al.

skillz uninstall --hook <fzf matches> --agent claude

skillz install <org/repo | path> # a wrapper to install the marketplaces as well as npx skills

# for plugins that have many skills, allow the argument --all or --skill one,two,three or --skill *commit (a basic regex or similar match on the string that will install only skills that match... we will provide the list of those matches and a simply (y/n) and even a basic list picker.
skillz uninstall <skill, fzf match on what is currently install>

# example:
matt pocock skills [mattpocock/skills]         *
killall skills     [killallgit/killall-skills] *
speckit            [etc/etc]
...etc

```

```
# completely remove the skill

skillz uninstall|rm|delete <name>
```

This should all work the same for hooks, plugins, any augmentation.

When i remove them for a project or globally i want to make sure they are totally removed so if it was installed via npx skill or through the marketplace, it gets properly removed.

On the hooks, we need a smart way to identify hooks that belong together. so that we can figure out if they are managed by a plugin/marketplace thing or were installed via some other tool. lets make it so that we can do `uninstall --hook <fzf match>` or `uninstall --agent <fzf match>... we can look at the hooks and the command that is used to run them to group them by the script/bin they call.

The goal is to be able to easily remove and add anything to my global or project level harness augmentations. These hooks and skills dont even need to be backed up when we remove. We just want to validate that after we remove a hook, the json, toml or whatever is valid still.

everything should be able to be fuzzy matched and then presented in a list of multi-select options.
