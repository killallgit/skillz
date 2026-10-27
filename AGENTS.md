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

1. Prefer CodeRabbit. `.coderabbit.yaml` makes it review only PRs labelled `needs-review`, and
   not again on later pushes, because the plan allows about one review an hour. Push first, then
   add the label (`gh pr edit N --add-label needs-review`) once the PR is ready, and confirm it
   reviewed the current HEAD commit (the `wait-for-action` skill with `--coderabbit`; without
   the label its status reads `Review skipped`, which is not a review). After fixing findings, comment `@coderabbitai review` to
   review the new commits, if the hour allows.
2. If CodeRabbit is unavailable or rate-limited, run a local review in a fresh
   context: a reviewer agent that did not write the change, given the diff, this file,
   `DESIGN.md`, `GLOSSARY.md`, and the originating bead. The author's own pass does not
   count.

Verify each finding against the code; fix what is valid and say why the rest were skipped.

See `DESIGN.md` for module ownership and execution rules; `README.md` documents
commands and interaction.
