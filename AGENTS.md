# Agent instructions

Use Beads (`bd`) for durable task tracking. Read `.agents/skills/beads/SKILL.md`
and run `bd prime` when starting or recovering context. Create and claim work before
editing; close completed issues after validation. Store project memory with
`bd remember`.

Issues live in `.beads/dolt/`. The JSONL export is passive; normal sync uses
`bd dolt push/pull`. Run Git operations—including add, commit, push, and merge—or
remote sync only when explicitly requested.

## Working conventions

- Prefix each shell command with `rtk`, including every command in a chain.
- Use non-interactive file operations: `cp -f`, `mv -f`, `rm -f`, and their recursive
  forms. Use `ssh`/`scp -o BatchMode=yes` and `HOMEBREW_NO_AUTO_UPDATE=1` for brew.
- Keep decisions pure and effects explicit. Inject `Env`, `CommandRunner`,
  `Prompter`, and harnesses into flows that need them.
- Keep agent-specific paths, formats, and commands in `crates/skillz-harness/src/agents/`.
  Imports go one way: `skillz-harness` (its own crate) < `src/app` < `src/tui` < `src/cli`;
  `tests/architecture.rs` guards the last three.
- Discovery reads native configuration and produces typed installed items with
  removal plans. File transformations are pure; execution applies the plans.
- Tests assert on typed behavior, actions, parsed contents, or tree structure.
  Avoid assertions on messages, errors, and rendered labels. Credential-redaction
  tests may inspect text because hiding credentials is the behavior under test.
- Keep comments that explain constraints and invariants. Prefer descriptive names
  and direct control flow over narration, speculative hooks, or test-only helpers.

## Validation

Before reporting implementation complete, run and report the entire required suite;
scoped checks do not substitute:

```sh
rtk task dev
rtk task test
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo llvm-cov --workspace --summary-only
```

Keep line coverage above 75%. The test task enforces a five-minute process-tree
limit. Smoke-test removal against a copied home, never the real home.

## Review before merge

After finishing work and opening a PR, decide whether it needs an independent review.
Documentation-only and small, mechanical changes do not. Anything that changes behavior,
safety, or module boundaries does, and it is not merged before one is done and its
findings are resolved.

See `DESIGN.md` for module ownership and execution rules; `README.md` documents
commands and interaction.
