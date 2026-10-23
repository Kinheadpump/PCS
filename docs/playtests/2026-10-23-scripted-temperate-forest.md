# Playtest: scripted run, temperate forest

This run is not a human playtest. A script played it (test `first_year_scripted_runs` in `crates/sim/tests/sprint7.rs`, started with `--ignored`). It gives no scores. The playtests of the owner replace it.

- Tester: scripted player (agent). No human.
- Build: `sprint-7-sim`, commit 365a13e (Sprint 7 sim and UI).
- Seed: the test preset `temperate forest`.
- Duration: 0 real minutes (headless). 36 (day 6 of winter, year 1) game days.
- Script: Farm zone, kitchen with keep 30, hearth, gathering and cutting every 10 days.

## Result

Failed on day 36 (day 6 of winter, year 1).

Deaths and causes:

- A colonist aged 34 froze to death.
- A colonist aged 25 froze to death.
- A colonist aged 30 froze to death.
- A colonist aged 42 froze to death.
- A colonist aged 20 froze to death.
- A colonist aged 18 froze to death.
- A colonist aged 40 froze to death.

## Decisions changed by a site

None. The script does not read the site.

## Scores

Not given. A script cannot score.

## Notes

The first harvest was 5 units. Frost killed 8 crop plants. The first winter began with 3 colonists and food for 72 days. The script built no walls, so the hearth heated no room. The cause of each death is cold, not hunger.

Food was never short. Cold killed the colony. This is a test of the script, not a proof that the winter is fair.
