# Agent instructions

Use Beads (`bd`) for durable task tracking. Read `.agents/skills/beads/SKILL.md`
and run `bd prime` when starting or recovering context. Create and claim work before
editing; close completed issues after validation. Store project memory with
`bd remember`.

Issues live in `.beads/dolt/`. The JSONL export is passive; normal sync uses
`bd dolt push/pull`. Run git operations and remote sync only when explicitly requested.

## Working conventions

- Prefix shell commands with `rtk`.
- Use non-interactive file operations: `cp -f`, `mv -f`, `rm -f`, and their recursive
  forms. Use `ssh`/`scp -o BatchMode=yes` and `HOMEBREW_NO_AUTO_UPDATE=1` for brew.
- Keep decisions pure and effects explicit. Inject `Env`, `CommandRunner`,
  `Prompter`, and adapters into flows that need them.
- Keep agent-specific paths, formats, and commands in `src/agents/`.
- Discovery reads native configuration and produces typed installed items with
  removal plans. File transformations are pure; execution applies the plans.
- Tests assert on typed behavior, actions, parsed contents, or tree structure.
  Avoid assertions on messages, errors, and rendered labels. Credential-redaction
  tests may inspect text because hiding credentials is the behavior under test.
- Keep comments that explain constraints and invariants. Prefer descriptive names
  and direct control flow over narration, speculative hooks, or test-only helpers.

## Validation

```sh
rtk task dev
rtk task test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo llvm-cov --summary-only
```

Keep line coverage above 75%. The test task enforces a five-minute process-tree
limit. Smoke-test removal against a copied home, never the real home.

See `DESIGN.md` for module ownership and execution rules; `README.md` documents
commands and interaction.
