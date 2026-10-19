# Agent rules

Canonical rules for all AI agents (D-031). Specifications are in `docs/`. Change a fact only in its canonical document:

- Game rules (`R-nnn`), glossary, scope: `docs/GAME_DESIGN.md`
- Architecture, dependencies, performance targets (`P-nn`): `docs/TECHNICAL_DESIGN.md`
- Sprints, Definition of Done, backlog: `docs/ROADMAP.md`
- Decisions, open questions, assumptions: `docs/DECISIONS.md`

Write all documents in ASD-STE 100 Simplified Technical English. Use the glossary names.

## Commands

```
cargo fmt --all --check                                  # required
cargo clippy --workspace --all-targets -- -D warnings    # required
cargo test --workspace                                   # required
cargo run -p game                                        # start the game
cargo run -p tools -- hash --seed 1                      # headless tools
UPDATE_GOLDEN=1 cargo test --workspace                   # only if the task permits a hash change
```

Never report that a check passed if you did not run it. Report each check that could not run.

## Workflow

1. Work only on tasks in the current sprint in `docs/ROADMAP.md`.
2. If a rule or a request is unclear, use the `owner-interviewer` subagent to prepare questions for the owner. Ask early: a wrong direction costs more later. Continue with the recommended assumption until the owner answers.
3. Write the test first. Put the rule ID in the test name: `r005_food_rots_twice_as_fast_above_25c`.
4. Put new ideas in the backlog (`docs/ROADMAP.md` §9), not in the sprint.
5. Report the document changes that your work needs. The lead agent edits `docs/`, `README.md`, `AGENTS.md`, and `CLAUDE.md`.

## Code rules

- `base`, `worldgen`, `sim`: no floats, no `HashMap` or `HashSet`, no file I/O, no time, no threads, no `unsafe`. Use the `base` RNG. Sort with an explicit tie-break.
- Only `game` uses `macroquad`. The UI reads `&Sim` and sends `Command` values. It never changes `Sim`.
- Each player-visible value is a sum of named contributions (why chain).
- Game numbers go in `assets/content/*.ron` as integers, not in code.
- No `unwrap()` or `expect()` on data from files or saves.
- No ECS, no plugin systems, no traits with one implementation.
- No new crate without a record in `docs/TECHNICAL_DESIGN.md` §13 and owner approval.
- A golden hash update goes in its own commit with the reason.

## Git

- Remote: `https://github.com/Kinheadpump/PCS`.
- Use CLI tools for external services: `gh` for GitHub (issues, PRs, CI runs, releases), and the matching CLI (`aws`, `gcloud`, `sentry-cli`) for other services.
- One branch for each task. Do not push to `main` without an instruction from the owner.
- Only one agent edits a file at a time.
- Never merge a PR until every check on its head commit has finished with the result `success`. Read each job result from the Actions jobs list for that commit, not only from the PR check list. That list can show a finished job as `in_progress`. If a check is not finished, wait and read it again. If a check hangs or is cancelled, re-run it. Never merge to get past a pending, hung, or failed check.
