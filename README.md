# Ruinstead (working title)

A lightweight, top-down, pixel-art colony simulation in Rust.

You start a colony in a procedurally generated fantasy world. The world has thousands of years of generated history. Ruins, old mines, roads, legends, and sealed dangers from that history change the land where you live. Temperature, materials, and resources obey simple and consistent rules. Failure is likely. The goal is to survive, adapt, and see how far the colony gets before it collapses.

## Current status

- **Phase:** 0, Foundation. **Sprint:** 0, not started.
- The concept and the plan are complete (draft 0.1). The plan has 17 sprints (S0 to S16).
- No code exists yet. Sprint 0 creates the Cargo workspace.
- Open decisions are in `docs/DECISIONS.md` §1.

## Documents

| Document | Contents |
|---|---|
| `docs/GAME_DESIGN.md` | The game concept, rules, and scope. |
| `docs/TECHNICAL_DESIGN.md` | Architecture, modules, dependencies, performance targets, and tests. |
| `docs/ROADMAP.md` | Phases, sprints, milestones, Definition of Done, and backlog. |
| `docs/DECISIONS.md` | Decisions, open questions, and assumptions. |
| `AGENTS.md` | Permanent rules for AI agents that work on this project. |
| `CLAUDE.md` | Imports `AGENTS.md` for Claude Code. |

Each subject has one canonical document. Change information only in its canonical document.

## Setup

1. Install Rust with `rustup` (https://rustup.rs).
2. The workspace file `rust-toolchain.toml` (from Sprint 0) selects the toolchain. `rustup` installs it automatically.
3. Install the components: `rustup component add rustfmt clippy`.

No C compiler or other system library is necessary.

## Development commands

These commands work after Sprint 0.

| Command | Action |
|---|---|
| `cargo run -p game` | Start the game. |
| `cargo run -p game --release` | Start the game with optimizations. |
| `cargo run -p tools -- <command>` | Run a headless tool, for example `hash --seed 1`. |
| `cargo run -p tools --release -- bench` | Run the performance benchmarks. |
| `cargo fmt --all --check` | Check the formatting. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Run the lint checks. |
| `cargo test --workspace` | Run all tests. |

The required checks for each change are in `docs/TECHNICAL_DESIGN.md` §11.2. The Definition of Done is in `docs/ROADMAP.md` §2.

## Repository

`https://github.com/Kinheadpump/PCS`

## Project structure

The planned structure is in `docs/TECHNICAL_DESIGN.md` §3.

## License

Not decided yet.
