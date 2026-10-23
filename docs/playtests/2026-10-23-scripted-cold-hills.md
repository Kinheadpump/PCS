# Playtest: scripted run, cold hills

This run is not a human playtest. A script played it (test `first_year_scripted_runs` in `crates/sim/tests/sprint7.rs`, started with `--ignored`). It gives no scores. The playtests of the owner replace it.

- Tester: scripted player (agent). No human.
- Build: `sprint-7-sim`, commit 365a13e (Sprint 7 sim and UI).
- Seed: the test preset `cold hills`.
- Duration: 0 real minutes (headless). 2 (day 2 of spring, year 1) game days.
- Script: The same script.

## Result

Failed on day 2 (day 2 of spring, year 1).

Deaths and causes:

- Seven colonists froze to death, aged 18 to 42.

## Decisions changed by a site

None. The script does not read the site.

## Scores

Not given. A script cannot score.

## Notes

The air was about -17 °C at the start. Hypothermia grows by 10 each hour and kills at 100, so an unsheltered colony dies in about 10 hours. No hearth was finished in that time.

A player must have a warm room in the first hours on this preset. Check if this is too hard (see DECISIONS).
