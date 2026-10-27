---
name: pre-merge-review
description: Use after opening a PR that changes behavior, safety, or module boundaries and before merging it.
user-invocable: false
metadata:
  internal: true
---

# Pre-merge review

In step 2, “this file” means the repository's `AGENTS.md`.

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
