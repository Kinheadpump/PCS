# Playtest: scripted run, dry plain

This run is not a human playtest. A script played it (test `first_year_scripted_runs` in `crates/sim/tests/sprint7.rs`, started with `--ignored`). It gives no scores. The playtests of the owner replace it.

- Tester: scripted player (agent). No human.
- Build: `sprint-7-sim`, commit 365a13e (Sprint 7 sim and UI).
- Seed: the test preset `dry plain`.
- Duration: 0 real minutes (headless). 10 (day 10 of spring, year 1) game days.
- Script: The same script.

## Result

Failed on day 10 (day 10 of spring, year 1).

Deaths and causes:

- Seven colonists died of heat, aged 18 to 42.

## Decisions changed by a site

None. The script does not read the site.

## Scores

Not given. A script cannot score.

## Notes

The script built no shade. All colonists died of heat on day 10.

The same finding as cold hills, with heat. Check if the start gives the player enough time to react.
