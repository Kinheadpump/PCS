# Game Design

Status: Draft 0.1. The lead approved it. Open questions remain (see `DECISIONS.md` §1). No gameplay code exists yet.

This document is the canonical source for the game concept, the game rules, the glossary, and the scope. Technical details are in `TECHNICAL_DESIGN.md`. The phase of each system is in `ROADMAP.md`. Decision records (`D-nnn`), open questions (`Q-nnn`), and assumptions (`A-nnn`) are in `DECISIONS.md`.

Writing rules:

- This document uses ASD-STE 100 Simplified Technical English.
- Numbered rules (`R-nnn`) are the testable game rules. Tests use the rule ID in their name, for example `r005_food_rots_twice_as_fast_above_25c`.
- All numbers are initial tuning values. The numbers live in the content data. If you change a number in the content data, change it here too.

## 1. Identity

**Working title:** Ruinstead (Q-001, answered).

**Genre:** Single-player colony simulation. Top-down 2D. Pixel art. Real time with pause. Mouse and keyboard. PC.

**Central experience:** The player manages a colony in the present and keeps it alive. Cold, hunger, disease, fire, beasts, and attackers put pressure on the colony. The player sees the warning signs, makes plans, and responds (D-035).

The land has a long history, but the game does not tell it as a story. The history is implied: it decides where the ore, the ruins, the roads, and the dangers are. The player sees the results, not the text.

The player gives orders but does not control the colonists directly. The colonists do the work. The environment and the history create the problems.

**Tone:** Grounded fantasy with low magic. Magic exists only as old relics and strange places. There are no spells. Text is short and serious, with some dark humor (Q-006, answered).

## 2. Design pillars

Use these pillars to accept or reject each feature.

1. **The colony in the present is the game.** Each system must give the player a problem to manage now or a choice to make now. History is implied. It leaves physical results on the map (ore, ruins, roads, hazards, enemies) but no story to read. If a generated fact cannot change a player decision, do not generate it.
2. **Physical rules are consistent.** Temperature, materials, fuel, and soil obey simple rules. The player can learn the rules. Wood burns. Stone walls keep heat. Food rots in warm air. Each year has a winter.
3. **The player can find the cause.** The player finds the causes of each important result in a maximum of two clicks. The complexity is in the interactions, not in the interface.
4. **Crises are expected, and failure ends the run.** Many different crises can hit the colony (§10.1). Each crisis has a warning sign and more than one player response. When the colony fails, the run ends.
5. **Each system connects to other systems.** A new system must interact with a minimum of two existing systems.

## 3. Glossary

Use these names in all documents, code, and messages.

### 3.1 World and history

| Term | Meaning |
|---|---|
| World | The full generated map of the planet surface. A grid of world cells. |
| World cell | One cell of the world map. It has height, temperature, moisture, biome, geology, and deposits. |
| Region | A group of world cells. The history simulation uses regions. |
| Civilization | A historical people with a culture. Civilizations exist only in the history in the MVP. |
| Site | A place that history made: ruin, tomb, battlefield, old mine, shrine, lair, or sealed site. |
| Sealed site | A site that contains a dormant threat. |
| Dormant threat | A creature that sleeps in a sealed site until the colony opens the site. Examples: guardian, bound beast. |
| Site record | The data for one site that history gives to local map generation. |
| Relic | A rare item from a site. It has high value and sometimes a special effect. |
| Salvage | Usable items in a site: stone blocks, metal scrap, tools. |
| Lore fragment | An item that holds knowledge. A colonist reads it to unlock a recipe or to get a legend. |
| Legend | A text that points to a site or a resource. It has a reliability: "certain" or "rumor". |

### 3.2 Local map

| Term | Meaning |
|---|---|
| Local map | The detailed tile map where the colony lives. Local map generation makes it from one world cell. |
| Tile | One square of the local map. One tile is approximately 1 m². |
| Embark | The act of choosing a world cell and starting a colony there. |
| Embark summary | The data that local map generation reads: climate, rock layers, deposits, depletion, roads, and site records. |
| Mountain mass | A connected area of natural rock wall tiles. |
| Room | A closed area with walls, doors, and a roof on each floor tile (R-020). |
| Stockpile zone | A player-defined area where colonists store items. |
| Blueprint | A planned structure. Colonists build it with materials. |
| Workshop | A building where colonists make items with recipes. |
| Hearth | A building that burns fuel and heats a room. |
| Connectivity region | A set of tiles that a colonist can walk between. Technical term. |

### 3.3 Colonists and creatures

| Term | Meaning |
|---|---|
| Colonist | One simulated person in the player colony. |
| Skill | A colonist ability with a level from 0 to 10, for example mining. |
| Trait | A permanent colonist property that changes rules, for example Hardy. |
| Need | A colonist value that decreases over time: hunger, rest, warmth. |
| Condition | A health state: starving, hypothermia, heatstroke, wounded, exhausted, or a disease. |
| Disease | A condition with a severity from 0 to 100: food poisoning, wound infection, winter fever (R-060). |
| Sick room | A room that the player marks for sick colonists (R-064). |
| Herbal medicine | An item from gathered herbs. A doctor uses it to treat a disease (R-064). |
| Mood | A colonist value from 0 to 100. It is the sum of mood modifiers. |
| Mood modifier | One named contribution to mood, for example "cold bedroom −6". |
| Comfort band | The temperature range in which a colonist is comfortable (R-001). |
| Creature | A non-colonist animal or monster. |
| Game animal | A creature that colonists can hunt. It does not attack. |
| Beast | A creature that attacks colonists. |
| Draft | A player order that puts a colonist under direct control for combat. |
| Rival colony | A living settlement of a historical civilization near the embark cell. It can raid the colony (R-070). |
| Raid | An attack by a group of fighters from a rival colony (R-071). |
| Scout | A rival fighter that comes 1 game day before a raid. |
| Tribute | Goods that the player pays to stop a raid (R-072). |

### 3.4 Player tools and interface

| Term | Meaning |
|---|---|
| Designation | A player order on a tile or an object: cut, mine, gather, sow, harvest, excavate, build, haul. |
| Job | A unit of work that one colonist does. Designations and production orders make jobs. |
| Work priority | A number from 1 (highest) to 4, or "off", for each colonist and each job type. |
| Production order | A player order for a workshop, for example "keep 30 meals". |
| Recipe | The inputs, the workshop, the work time, and the outputs of one product. |
| Knowledge | The set of recipes that the colony can use. |
| Notice | A colony value. It shows how much the colony attracts danger (R-030). |
| Wealth | The total value of all colony items and structures. |
| Fertility | A tile value that multiplies crop yield. |
| Weather offset | A daily temperature change that makes cold days and warm days (R-003). |
| Drought | A period with 5 or more days without rain (R-025). |
| Irrigation ditch | A dug channel that carries water from a water tile (R-025). |
| Why chain | The list of named contributions to a value, shown in the inspector. |
| Overlay | A map view that shows one value for each tile, for example temperature. |
| Inspector | The panel that shows the values and why chains of a selected object. |
| Ledger | The panel that shows stock, trend, and days remaining. |
| Alert | A message about an important event, with a level: critical, warning, or information. |
| Chronicle | The record of colony events in short text. |
| Legacy score | The score at the end of the run. |
| Ironman | A save mode with one save file that the game writes automatically. |
| Tick | One step of the simulation. |
| Game day | 24 game hours. |

## 4. Core gameplay loop

The loop: **Look at the map → Plan → Colonists work → Conditions change → Read the results → Change the plan.**

### 4.1 Short term (seconds to minutes)

The player does these actions:

- Designate trees to cut, rock to mine, plants to gather, fields to sow, and stockpile zones.
- Place blueprints for walls, floors, doors, roofs, beds, hearths, and workshops.
- Set work priorities for each colonist.
- Set production orders, for example "keep 30 meals" or "make 5 axes".
- Respond to alerts: a colonist is cold, food rots, a beast comes near.
- Inspect a colonist or a tile to see the why chain of a value.

### 4.2 Medium term (one season)

The player prepares for the next season:

- Collect food and fuel for winter. The ledger shows the days remaining.
- Build insulated rooms for sleep and storage.
- Excavate a site. Get salvage, lore fragments, or relics. Accept the risk of injury or of a dormant threat.
- Build the next production step, for example a charcoal kiln, then a smelter, then iron tools.

### 4.3 Long term (game years)

- Grow from a camp to a village to a fortified town.
- Unlock knowledge from lore fragments.
- Respond to long threats: an open sealed site, local resources that become depleted.

## 5. First playable situation

| Item | Value |
|---|---|
| Colonists | 7. Each has a name, an age, 1 trait, and 2 main skills at level 3 to 5. The other skills are at level 0 to 2. |
| Local map | 256 × 256 tiles (D-024). |
| Start date | Day 1 of spring, year 1. |
| Food | 420 simple meals (7 colonists × 30 game days × 2 meals). |
| Tools | 3 stone axes, 2 stone picks. |
| Seeds | Seeds for 2 crop types that grow in the local climate. |
| Clothes | Each colonist wears simple clothes (insulation 5 °C). |
| Other | 20 logs. |
| Visible site | A minimum of 1 site on the local map. It is visible at the start (R-041). |

The player must do these tasks in the first year:

1. Build shelter: walls, a roof, beds, and a hearth before the nights become cold.
2. Start a food supply: fields and gathering. Build a cool storage room.
3. Collect fuel for winter. The nearby trees decrease.
4. Replace tools when they break.
5. Excavate the visible site. Find salvage and a minimum of 1 lore fragment.
6. Survive the first winter.

## 6. Time and calendar

- A year has 4 seasons: spring, summer, autumn, winter. Each season has 10 game days. A year has 40 game days. (Q-004, answered: start value, playtests tune it)
- At speed 1×, one game day is approximately 60 real seconds. One year is approximately 40 real minutes.
- Speeds: pause, 1×, 3×, 6×.
- By default, the game pauses on critical alerts. The player can change this.
- The tick rates are in `TECHNICAL_DESIGN.md` §6.1.

## 7. Colonists

### 7.1 Unit of control (D-001)

The simulation updates each colonist separately. The player controls colonists indirectly with designations, work priorities, and production orders. Direct control is available only for drafted colonists in combat.

Scale targets:

- MVP: 30 to 50 colonists is the normal case. The game must work with 100 colonists.
- Phase 6: 300 colonists. Idle or distant colonists update less frequently.

This section is the canonical source for colonist scale.

### 7.2 Colonist data

Each colonist has:

- Name, age, and sex.
- Skills: mining, woodcutting, construction, farming, cooking, crafting, hunting, combat, medicine. Each skill has a level from 0 to 10.
- Traits: 1 trait in the MVP. MVP traits: Hardy (comfort band lower limit −5 °C), Glutton (hunger decreases 50% faster), Bold (danger mood modifiers are 50% smaller), Lazy (work speed −15%), Strong (carry capacity +50%).
- Needs: hunger, rest, warmth.
- Mood and health.
- Equipment: clothes and one tool or weapon.

**R-010 Skill gain:** Each completed job of a skill type adds experience. Each 1,000 experience points add 1 level, to a maximum of 10. Work speed is `100% + 5% × level`.

### 7.3 Needs

**R-011 Hunger:** Hunger starts at 100. It decreases by 50 each game day. At 30 or less, the colonist goes to eat. One meal adds 50, to a maximum of 100. At 0, the colonist gets the condition "starving". A starving colonist loses 2 health each game hour.

**R-012 Rest:** Rest starts at 100. It decreases by 6 each awake game hour. At 25 or less, the colonist goes to sleep. Sleep adds 12 each game hour in a bed, or 8 on the ground. At 0, the colonist gets "exhausted": work speed −50%. The colonist sleeps where it is.

**R-001 Comfort band:** The comfort band is 10 °C to 26 °C. Clothes insulation lowers the lower limit. Example: simple clothes (insulation 5 °C) give a lower limit of 5 °C. The Hardy trait lowers the lower limit by 5 °C more.

**R-013 Warmth:** Each game hour, the simulation compares the tile temperature with the comfort band.

- Inside the band: hypothermia and heatstroke each decrease by 20, to a minimum of 0.
- Below the lower limit: mood modifier "cold" −5.
- More than 10 °C below the lower limit: hypothermia increases by 10.
- More than 10 °C above the upper limit: heatstroke increases by 10.
- Hypothermia or heatstroke above 50: the colonist loses 5 health each game hour.

Example: a colonist in simple clothes at −10 °C (lower limit 5 °C) gets hypothermia 50 after 5 game hours. The colonist dies after approximately 25 game hours.

### 7.4 Mood

**R-014 Mood:** Mood is 50 plus the sum of all active mood modifiers, limited to 0 to 100. The inspector shows each modifier.

MVP modifier sources: needs, temperature, room beauty, food type, death of a colonist, danger, and relics.

**R-015 Mood effects:**

| Mood | Effect |
|---|---|
| 70 to 100 | Work speed +10%. |
| 30 to 69 | No effect. |
| 15 to 29 | Work speed −20%. Each game hour, a 10% chance that the colonist stops work and walks around for 1 game hour. |
| 0 to 14 | Breakdown: the colonist stops work for 4 game hours. If mood stays below 15 for 5 game days, the colonist leaves the colony. |

### 7.5 Health

**R-033 Personality:** A colonist can have a social trait: introvert or extrovert. A colonist can have 1 life goal. A scenario map colonist has neither. (D-083)

**R-034 Wants:** Each game hour, a want gives a timed mood modifier (R-014) that has a name. An extrovert with company is glad, and an extrovert that is alone for 24 hours in a row is lonely. An introvert that is alone is glad, and an introvert in a crowd for 12 hours in a row is crowded. A met life goal adds a modifier. A life goal that is not met adds a negative modifier after a patience time of 20 days. (D-083)

**R-035 Opinion:** Two colonists that are near each other have an opinion of each other. The opinion is the sum of named parts: time together, shared sleep, quarrels, temperament, and chemistry. The opinion gives a standing: acquaintance, friend, rival, or enemy. (D-084)

**R-036 Bonds:** A friend, a rival, or an enemy that is near a colonist adds a named mood modifier. When a friend dies, the friend gets a stronger modifier than other colonists. Two free adults with a high opinion become partners, and the death of a partner gives the strongest modifier. (D-084)

**R-037 Ages:** A colonist ages 1 age year after a set number of game days (`family.ron`). A colonist below the adult age is a child: it eats and sleeps but does not work and cannot be drafted. From the elder age, a colonist can die of old age. The chance rises with each age year. (D-085)

**R-038 Children:** A partner pair with a woman and a man can have a child if the woman is fertile, the pair is in good mood, the colony has food and a free bed, and the woman has no pregnancy or recent birth. The pregnancy lasts a set number of game days. A birth gives both parents a good mood modifier. When a parent or a child dies, the family gets a large bad mood modifier. (D-085)

**R-039 Homes:** Each colonist has an assigned bed. A pair and its children live in the same room if it has enough beds. A single colonist prefers a room with few beds. A tired colonist sleeps in the assigned bed when it is free. (D-085)

**R-040 Colony growth:** The chance of migrants (R-017) is the sum of the base chance, the wealth, the average mood of the colonists, and the number of friend and partner pairs. Each part is named. A colonist whose life goal fails for a long time and whose mood is low leaves the colony. A child never leaves for this reason, and the last colonists stay. (D-086)

**R-016 Health:** Health is from 0 to 100. At 0, the colonist dies. Each game hour without a condition, health increases by 2 in a bed and by 1 in other places.

MVP conditions: starving, hypothermia, heatstroke, exhausted, wounded, and the diseases in §7.7. A wounded colonist loses 1 health each game hour until the colonist sleeps in a bed for 8 game hours. Disease and medicine are not in the MVP.

### 7.7 Disease and medicine

**R-060 Disease:** A disease is a condition with a severity from 0 to 100. Each disease type has these values in the content data: severity at start, severity change each game day without treatment, severity change each game day in a bed, severity change each game day with treatment, work speed penalty, and contagion. At severity 0, the disease ends. Above severity 50, the colonist loses 2 health each game hour. The inspector shows each severity change as a why chain.

**R-061 Food poisoning:** When a colonist eats, the chance of food poisoning is:

- Raw food: 20%.
- Cooked food: 5% − 0.5% × cook skill level, to a minimum of 0%.
- Food with rot value 50 or more: +30%.

Food poisoning starts at severity 40 and decreases by 20 each game day. Work speed −50%. It is not contagious.

**R-062 Wound infection:** Each game day, a wounded colonist gets a wound infection with this chance: 15% if the colonist did not sleep in a bed, 5% if the colonist slept in a bed, 1% if a doctor treated the wound. Severity starts at 20. It increases by 15 each game day without treatment and decreases by 20 each game day with treatment and bed rest.

**R-063 Winter fever:** Winter fever is contagious.

- Each game hour, a colonist on a tile below the lower comfort limit has a 0.1% chance to catch it.
- Each night, each sick colonist infects each other colonist that sleeps in the same room. The chance is 20%, or 10% if the room temperature is inside the comfort band.
- Severity starts at 20. It increases by 10 each game day, decreases by 15 each game day in a warm bed, and decreases by 30 each game day with herbal medicine.
- After recovery, the colonist cannot catch winter fever for 20 game days.

**R-064 Doctor and sick room:** A doctor job treats one sick or wounded colonist in a bed. It takes 1 game hour and uses 1 herbal medicine. The treatment lasts 1 game day. The success chance is 60% + 4% × medicine skill level. A colonist makes herbal medicine from gathered herbs at a workshop (start knowledge). The player can mark a room as a sick room. Sick colonists sleep only in a sick room if a bed is free there. Contagion happens only between colonists in the same room.

### 7.6 Migrants

**R-017 Migrants:** Once each game day, the simulation checks for migrants. Migrants can arrive only when food days remaining are 10 or more and a minimum of 1 bed is free. The chance is 5% plus 1% for each 1,000 wealth, to a maximum of 20%. A group has 1 or 2 migrants. The game pauses and asks the player to accept or refuse. Migrants that the player refuses leave the map.

Births are not in the MVP. They come with relationships in a later phase (§7.8).

### 7.8 Colonist life (later phase, Q-014)

This section is a plan. It has no rule IDs until the owner approves it. The MVP has only needs, mood, traits, and migrants. The MVP slice is bonds and a background line for migrants (`ROADMAP.md` §7.1).

- **Wants:** Each colonist has 1 life goal, for example "own a room" or "become a master cook", and a few short wants. A met want adds a named mood modifier. An unmet want adds a negative one.
- **Relationships:** Two colonists have an opinion of each other. Work, sleep, meals, and shared danger raise it. Theft and neglect lower it. States: friend, rival, partner. The inspector shows each contribution.
- **Families:** Partners can have children. A child grows to an adult and then ages. Children eat and sleep but do not work. The death of a partner, a child, or a parent gives a large mood modifier.
- **Colony growth:** The colony can grow from inside. Good mood and strong bonds increase the migrant chance (R-017). A colonist whose goals fail for a long time can leave.
- **Personality:** A colonist is an introvert or an extrovert. An extrovert gets a mood modifier from time with others. An introvert gets one from time alone or in a private room.
- **Jobs:** Each colonist wants work. A colonist picks the job by colony need and by how many colonists already do it. Without work, the colonist gets a negative mood modifier.
- **Other colonies and races:** Colonists can have friends and enemies in rival colonies. A few other races (3 or 4) differ in content values only: age span, traits, and comfort band. Each race pair has a rule for attraction and for offspring (Q-015).
- **Limits:** No family trees for historical figures (§16.3). No new crate. All numbers go in content files.

## 8. Environment

### 8.1 Weather and outdoor temperature (D-018)

Each world cell has an annual mean temperature, a seasonal amplitude, and a moisture value.

**R-002 Outdoor temperature:** The outdoor temperature is one value for the full local map. Each game hour:

`outdoor = mean + amplitude × season_curve(day) + daily_curve(hour) + weather_offset(day)`

The curves are lookup tables in the content data.

**R-003 Weather:** The simulation calculates the weather for each game day from the game seed and the day number.

- The weather offset changes by a random value from −3 °C to +3 °C each day. It stays between −12 °C and +12 °C. Long negative series are cold periods.
- Rain: each day has a chance of rain. The chance is the world cell moisture (0 to 255) × 50% / 255.
- The forecast shows the true weather for the next 3 days.

### 8.2 Room temperature

**R-020 Room:** A room is a closed area. Walls, doors, or natural rock surround it. Each floor tile has a roof. A roof tile needs a wall or natural rock within 6 tiles (A-008). A room has a maximum of 400 tiles.

**R-021 Room insulation:** Room insulation (0 to 100) is the integer mean of the insulation of all wall, door, and rock tiles on its border, rounded down.

**R-022 Room temperature:** Each game hour:

`room += (reference − room) × (100 − insulation) / 400 + hearth_heat / room_tiles`

- The reference is the outdoor temperature. For a room with 75% or more natural rock on its border, the reference is the ground temperature (the annual mean of the world cell). This makes rock cellars useful.
- `hearth_heat` is 40 °C·tiles for each burning hearth.
- A hearth burns 1 fuel unit each 4 game hours. A colonist with a hauling job refuels it.
- An open door does not change the calculation in the MVP.
- Tiles that are not in a room use the outdoor temperature.

Example: a 16-tile room with insulation 60 and one hearth, at −10 °C outdoors, reaches approximately 15 °C.

### 8.3 Water

**R-004 Freezing:** Shallow water tiles freeze after 3 game days with a mean outdoor temperature below 0 °C. Colonists can walk on ice. Ice thaws after 3 game days with a mean above 0 °C. Deep water does not freeze in the MVP.

Colonists do not have a water need in the MVP. Water tiles block movement, except ice.

### 8.4 Food spoilage

**R-005 Rot rate:** Each food item has a rot value. Each game hour, the rot value increases by the base rate of the food × the rate factor:

| Tile temperature | Rate factor |
|---|---|
| Below 0 °C | 0 (frozen) |
| 0 °C or more, below 8 °C | 0.25 |
| 8 °C or more, below 25 °C | 1 |
| 25 °C or more | 2 |

Smoked food uses 10% of the normal base rate. At rot value 100, the item is rotten. Colonists do not eat rotten food.

### 8.5 Plants and crops

**R-006 Crops:** Each crop type has a minimum growth temperature, a maximum growth temperature, a frost kill temperature, and a growth time in game days. A crop grows only when the tile temperature is inside its band. It dies when the temperature is below its kill temperature. Yield is the base yield × fertility (50% to 150%).

**R-025 Drought:** The simulation counts the days without rain. At 5 or more days, a drought is active. A rain day ends it. During a drought:

- Crops on tiles that are not irrigated grow at 50% speed. Crops with the flag "dry soil" grow at normal speed.
- Gathered wild plants do not grow again.
- Fire spread chance (R-024) is doubled.

A tile within 4 tiles of a water tile or a connected irrigation ditch is irrigated. A colonist digs an irrigation ditch with a designation. A ditch tile is connected if it touches a water tile or a connected ditch tile. Colonists cannot walk on ditch tiles.

**R-026 Wildfire:** On each summer day with an active drought, there is a 10% chance of lightning. The lightning hits a random tree or grass tile and starts a fire with intensity 5 (R-024). Soil, stone, and ditch tiles do not burn, so the player can make firebreaks.

**R-007 Wild plants:** Each biome has wild plants in the content data, for example berry bushes and roots. A gather designation gives food or fiber. A gathered plant grows again after a time in game days. Wild plants do not grow in winter.

### 8.6 Materials

The game has approximately 30 materials in the content data (D-014). Only these properties exist:

| Property | Use |
|---|---|
| hardness | Mining time, work time, tool durability, weapon damage, structure durability. |
| density | Carry weight and haul time. |
| value | Wealth and Notice. Trade value later. |
| insulation | Room insulation for walls. Comfort band shift for clothes. |
| flammability | Fire spread chance. Fuel value. |
| beauty | Room beauty for mood. |
| smelts to | The output metal and the fuel cost for ore. |
| rot rate | Spoilage for food materials. |
| color | The palette color for sprite tinting. |

Material categories: stone, ore, metal, wood, soil, plant, cloth, leather, food, fuel.

Item durability and value come from the material and the item template.

### 8.7 Tools

**R-008 Tools:** A cut job needs an axe. A mine job needs a pick. A colonist without the tool takes one from a stockpile. Each completed job decreases tool durability by 1. Durability starts at `hardness × 10`. At 0, the tool breaks.

### 8.8 Fire

**R-023 Fire start:** Fire starts only from a burning hearth. Each game hour, each flammable item or object (bed, door, workshop) within 1 tile of a burning hearth has a 1% chance to start burning. Floors and roofs burn only when a fire spreads to them. The first 10 days of a run have no hearth fire.

**R-024 Fire spread:** A burning tile has fire intensity from 1 to 10. Each 10 ticks, it has a chance to spread to each adjacent flammable tile. The chance is `flammability × intensity / 10` percent. A burning tile uses its fuel. When the fuel is 0, the fire stops. Stone and soil do not burn. Rain stops all outdoor fire. A colonist with an extinguish job decreases intensity by 2 each game hour.

### 8.9 Disasters

Disasters come from the simulation, not from scripts. The full list is the crisis catalog (§10.1).

## 9. World and history

### 9.1 World generation

The world is a 256 × 256 grid of world cells. The generator makes, in this order: height, sea level, lakes, temperature, moisture, rivers, biomes, geology, deposits, regions, and history. The algorithms are in `TECHNICAL_DESIGN.md` §8.

At embark, the player sees the world map: biome, temperature range, rivers, roads, and known sites. The player chooses one world cell. The player types a world seed as a number, or the game selects one.

### 9.2 History (D-019)

The history simulation runs for 3,000 years by default. The player can select 1,000 to 5,000 years. It simulates civilizations on the region graph. It does not simulate persons, except for a limited number of notable figures.

The history makes civilizations, settlements, notable figures, artifacts, sites, beasts, and events.

**R-040 Map result rule:** Each event type must write a minimum of one of these results: a site, a map overlay (depletion, road, owner), or a civilization state. Do not add an event type without a map result.

### 9.3 History in gameplay (D-020)

The MVP has these five history features:

1. **Ruins with salvage and danger.** Each site has a type, a cause of destruction, a culture, and an age. These values control the content: salvage, relics, lore fragments, and occupants. Old ruins are partly buried. Colonists must excavate them.
2. **Depleted and untouched deposits.** Old mines in the history remove ore. Deposits near old mines are poor. Untouched deposits are rich. Old mine tunnels exist on the local map, so colonists can reach ore faster.
3. **Ancient roads.** Old roads cross the local map. They show the direction of other sites. Colonists move 30% faster on roads.
4. **Legends that point to lost artifacts.** A lore fragment can contain a legend. The legend is one line of text and a site marker on the local map. It is a lead to a reward, not a story.
5. **Sealed sites.** Some sites contain a dormant threat. The site has a warning legend. If colonists open it, the colony gets a large reward and the threat wakes up.

**R-041 Site presence:** Each local map has a minimum of 1 site. If the history gives no site in the embark area, local map generation adds a small ruin.

**R-042 Lore fragments:** Lore fragments are in containers in site templates. A colonist takes a fragment to a reading table and reads it for 4 game hours. Each fragment gives one fixed knowledge unlock or one legend. The site type selects it.

**R-043 Progression guarantee:** Each local map contains lore fragments for charcoal, fired clay, and smoked food. If the map has an iron deposit, it also contains the lore fragments for smelting and iron tools.

**R-044 Legend reliability:** A "certain" legend always points to a real site. A "rumor" legend points to a real site with a 50% chance. A false rumor points to a location with no site. The player sees the reliability.

Later features are in `ROADMAP.md` §7.

## 10. Crises and threats (D-004)

There is no AI storyteller and no scripted event deck. Crises come from the simulation: the weather, the world, the history, and the actions of the player.

### 10.1 Crisis catalog

Each crisis must have a cause in the simulation, a warning sign that the player can see before the crisis is serious, and a minimum of two player responses. Status: **MVP** (in the plan), **Later** (`ROADMAP.md` §7 or §9).

| Group | Crisis | Cause in the simulation | Warning sign | Player responses | Status |
|---|---|---|---|---|---|
| Weather | Cold period | Negative weather offset (R-003). | Forecast. Temperature alert. | Insulate rooms, more hearths, fuel stock, warm clothes, keep colonists inside. | MVP |
| Weather | Heat wave | Positive weather offset in summer (R-003). | Forecast. Heatstroke alert. | Rock rooms, work at night, cool storage for food. | MVP |
| Weather | Drought | Many days without rain. Crops grow slower. Wild plants do not grow again. Fire spreads faster. | Days without rain in the ledger. Crop growth why chain. | Irrigation ditch from a river, crops for dry soil, food stock, firebreaks. | MVP |
| Weather | Wildfire | Lightning on a dry summer day starts a fire outside. Uses R-024. | Drought warning. Smoke at the map edge. | Firebreaks, stone buildings, extinguish jobs. | MVP |
| Weather | Flood | Snow melt raises the river in spring. | River level forecast. | Build away from the river, dikes. | Later |
| Disease | Food poisoning | A colonist eats raw food or food with a high rot value. Low cooking skill increases the chance. | Rot values in the ledger. Sick colonist alert. | Cook all food, cool storage, remove old food, a better cook. | MVP |
| Disease | Wound infection | A wounded colonist without bed rest or treatment. | "Wounded" condition in the inspector. | Bed rest, a doctor job, herbal medicine. | MVP |
| Disease | Winter fever | Spreads between colonists who sleep in the same cold room. | First case alert. Number of sick colonists. | A separate sick room, warm bedrooms, isolation, herbal medicine. | MVP |
| Disease | Plague | Rats and dead bodies. | Rats in the storage. | Graves, clean storage, cats. | Later |
| Supply | Famine | Food stock goes to 0 (R-011). | Food days remaining in the ledger. | Rationing by priority, more fields, hunting, gathering. | MVP |
| Supply | Spoilage | Warm storage (R-005). | Rot values. Storage temperature. | Rock cellar, smoked food, eat old food first. | MVP |
| Supply | Fuel shortage | Hearths burn fuel (R-022). Trees decrease. | Fuel days remaining. | Charcoal, coal, replant trees, smaller heated rooms. | MVP |
| Supply | Tool shortage | Tools wear out (R-008). | Tool count in the ledger. | Crafting, better materials, salvage from sites. | MVP |
| Supply | Crop blight | Pests in large fields of one crop. | Crop health in the inspector. | Mixed crops, burn the infected field. | Later |
| Attack | Beasts | Notice (R-031). | Beast at the map edge. | Walls, doors, drafted defenders, lower Notice. | MVP |
| Attack | Dormant threat | Colonists open a sealed site. | Warning legend on the site. | Do not open it. Prepare defenses before opening. | MVP |
| Attack | Raid by a rival colony | Rival settlements on the world map. Their distance, the colony wealth, and Notice control raids. | A scout at the map edge 1 game day before the raid. | Walls, gates, drafted defenders, pay a tribute, hide colonists in a closed room. | MVP |
| Attack | Thieves | Rival settlements steal at night. | Missing items in the ledger. | Locked storage, guards. | Later |
| Colony | Mood breakdown and desertion | Low mood (R-015). | Mood alert and why chain. | Better rooms, food, rest, fewer dangers. | MVP |
| Colony | Fights between colonists | Traits and low mood. | Mood alert. | Separate the colonists. | Later |
| Structure | Fire | Hearths near flammable items (R-023, R-024). | Fire alert. | Stone walls, space around hearths, extinguish jobs. | MVP |
| Structure | Cave-in | Mining under a large unsupported rock area. | Cracks in the tile inspector. | Leave pillars, build supports. | Later |

### 10.2 Notice

**R-030 Notice:** Notice is the sum of these contributions. Each contribution has a location for the Notice overlay.

| Contribution | Value | Decrease |
|---|---|---|
| Wealth | wealth / 1,000 | — |
| Population | 2 × colonists | — |
| Noise | +1 for each completed mining or construction job | −10% each game day, rounded up |
| Site disturbance | +20 for excavation of a site, +10 for each relic taken, +50 for opening a sealed site | −1 each game day |

**R-031 Notice effects:**

- Each game day, a beast enters the map with a chance of Notice / 4 percent, to a maximum of 50%.
- Beasts in lairs on the map become active when Notice is 30 or more. No beast arrives and no lair wakes in the first 8 game days of the run.
- An awake dormant threat gets 1 strength level for each 25 Notice.

### 10.3 Creatures

- Game animals walk around the map. The player orders a colonist to hunt one (an attack order). A dead animal leaves meat and hide. A workshop makes leather from hide, and clothes from leather.
- Beasts attack colonists. They come from the map edge or from lairs.
- Rival colonies send raids (§10.5).
- Dormant threats wake up when colonists open a sealed site.

### 10.4 Combat (MVP)

**R-032 Melee:** The player can draft colonists. A drafted colonist obeys move and attack orders. MVP combat is melee only. Damage is `weapon base damage × (100% + 10% × combat level)` plus a roll from 0% to 20%. Damage decreases health. A hit that does 10 or more damage adds the condition "wounded". There are no body parts in the MVP. (Q-009, answered)

### 10.5 Rival colonies and raids

**R-070 Rival colonies:** Local generation selects up to 3 living settlements of historical civilizations near the embark cell. Each one has a strength (3 to 12 fighters) and a hostility from 0 to 100. Hostility is the base value from history (the aggression of its civilization, and the battles and razed settlements of that civilization in the embark area) + wealth / 1,000 + Notice / 2 + the result of earlier raids, from 0 to 100. The lore panel shows each rival colony with its strength and hostility.

**R-071 Raid:** Each game day after day 20 of the run, each rival colony raids with a chance of hostility / 20 percent, to a maximum of 5%. The raid has `strength × hostility / 100` fighters, to a minimum of 2. A scout comes to the map edge 1 game day before the raid and makes a warning alert. Raiders use melee combat (R-032). They attack colonists, break doors and walls (time from material hardness), and carry away items. They leave when half of the raiders are dead or down.

**R-072 Tribute:** When the raiders arrive, the game pauses. The player selects one choice:

- **Pay:** the colony gives goods with a value of 200 × the number of raiders. The player selects the goods. The raiders leave. Hostility decreases by 20.
- **Fight:** the raid starts. If the colony wins, hostility decreases by 10. If the raiders carry away items, hostility increases by 10.

The player cannot pay if the colony has too few goods. There is no other diplomacy in the MVP.

## 11. Progression

### 11.1 Knowledge (D-005)

There is no research tree. A colony knows a recipe from the start, or it unlocks the recipe with a lore fragment (R-042).

Start knowledge: stone tools, firewood, planks, stone blocks, simple meals, beds, hearth, simple clothes from fiber.

MVP knowledge unlocks:

| Unlock | Effect |
|---|---|
| Charcoal | Fuel that burns 3 times longer than firewood. Needed for smelting. |
| Fired clay | Bricks with insulation 70. Pots. |
| Smoked food | Meat or crops + firewood in a smokehouse. Rots at 10% of the normal rate. |
| Smelting | Iron bars from iron ore and charcoal. |
| Iron tools and weapons | Higher durability. Faster work. More damage. |

### 11.2 Material tiers

Wood and stone → fired clay and charcoal → iron → relic materials (only in sites).

### 11.3 Production chains in the MVP

- Log → plank, or log → firewood, or log → charcoal.
- Rock → stone block.
- Clay → brick.
- Iron ore + charcoal → iron bar → tool or weapon.
- Crop or meat → simple meal. Meat or crop + firewood → smoked food.
- Fiber → simple clothes. Leather → leather clothes (insulation 10 °C).

Tools wear out with use (R-008).

## 12. Failure and end of the run

**R-050 End of run:** The run ends when the colony has no colonists. Colonists that leave count as lost. The player can also abandon the colony.

At the end, the game shows the legacy score (years survived, peak population, crises survived, knowledge unlocked) and the chronicle as a short list of key events.

Save mode: ironman (D-032).

### 12.1 Why failure is likely but fair

- The player can see threats before they arrive: the 3-day forecast, beasts at the map edge, warning legends.
- Each death has a cause in the chronicle.
- The player selects the risk. Mining, excavation, and growth increase Notice.
- Difficulty comes from the choices of the player, not from a hidden timer.

## 13. Interface and readability

The interface has these elements:

- **Main view:** the local map in pixel art. Camera pan and zoom. Time controls.
- **Overlays:** one key for each overlay: temperature, fertility, deposits, rooms, history (sites, roads, depletion), Notice sources.
- **Inspector:** select a colonist, a tile, an item, a room, or a site. Each value in §13.1 shows its why chain.
- **Ledger:** stock of food, fuel, and materials with a trend. Food and fuel show days remaining.
- **Alerts:** Critical alerts pause the game: fire, attack, starvation, death. Warnings: low fuel, hypothermia, low mood. Information: migrants, lore found, knowledge unlocked. Click an alert to go to its location.
- **Work panel:** a grid of colonists and job types with work priorities.
- **Chronicle:** short text entries, grouped by season. Filter by colonist or site.
- **Lore panel:** known legends with reliability, known sites, and known recipes.

### 13.1 Values with why chains

| Value | Contributions |
|---|---|
| Mood | Each mood modifier (R-014). |
| Work speed | Skill, mood, exhaustion, trait, tool. |
| Tile temperature | Outdoor terms (R-002) or room terms (R-022). |
| Room insulation | Each border material and its tile count (R-021). |
| Crop growth | Temperature band, fertility, season. |
| Notice | Each contribution (R-030). |
| Days remaining | Stock and the mean use of the last 3 game days. |
| Health | Each active condition and its loss for each hour. |

Each sprint adds the why chain rows, alerts, and chronicle entries for its own systems.

## 14. Differences from other games

| Game | Difference |
|---|---|
| Dwarf Fortress | No z-levels in the MVP. An interface with why chains. History is something that the player explores and uses. A much smaller scope. |
| Songs of Syx | Tens of named colonists, not thousands of abstract citizens. Survival on one local map, not management of an empire. History and environment are more important than economy optimization. |
| RimWorld | No storyteller and no event deck. Threats come from Notice, the world, and history. Persistent generated history. Climate and materials are central. Low-technology fantasy. Long runs over many years. |

## 15. Example play session

1. The player embarks in a cold valley. The valley contains the Sunken Quarry. Its owners left it 400 years ago after a collapse.
2. Year 1: Stone is easy to get. Ore is poor, because the old miners took it. A lore fragment in the quarry office tells of "the second gallery, sealed after the collapse".
3. Year 2: The winter is hard. Firewood is low, because a past civilization cut the old forest. Only young trees grow. The player orders the colonists to open the seal.
4. The gallery contains a rich coal seam. The colony is warm for the first time.
5. The mining noise increases Notice. Pale cave crawlers come out of the gallery and attack a hunter. Edda, a miner, kills one. The chronicle records it.
6. Spring: Edda finds a tomb in the gallery. A warning legend tells of "the Grave of the Hundred". The player ignores it. The colonists take a relic.
7. Summer: A bound guardian wakes up. Edda dies at the gallery entrance. The mood modifier "friend died" makes Joss stop work. Food production decreases.
8. Year 3: The player gives the jobs to other colonists. The colony survives one more winter with smoked food.
9. Year 5: The guardian is stronger, because Notice is high. It breaks into the storehouse. A hearth fire spreads through the wooden walls. The last food burns. The colony collapses.

## 16. Scope

### 16.1 Systems in the MVP

- Colonists with needs, mood, health, skills, and traits.
- Jobs, work priorities, designations, construction, rooms, and stockpiles.
- Items with materials. Tools with durability.
- Weather, seasons, outdoor and room temperature, fuel, freezing water, crops, wild plants, spoilage.
- Production chains and knowledge unlocks from lore fragments.
- World generation, history, and embark.
- The five history features in §9.3.
- Notice, game animals, beasts, dormant threats, and melee combat.
- Rival colonies, raids, and tribute.
- Disease, the doctor job, herbal medicine, and sick rooms.
- Fire, cold periods, heat waves, drought, irrigation, and wildfire.
- Migrants.
- Overlays, the inspector with why chains, the ledger, alerts, the chronicle, and the lore panel.
- Save and load. The end-of-run screen.

### 16.2 Later systems

The phase of each later system is in `ROADMAP.md` §7. Each later system connects to the core loop:

| System | Connection to the core loop |
|---|---|
| Relationships and births | Deaths have more effect. The colony can grow from inside. Plan in §7.8. |
| Larger wars with rival civilizations | Rival colonies grow, make alliances, and attack with armies. Uses the raid system. |
| Ranged combat | Raiders and colonists with bows. Walls and towers become more important. |
| Trade caravans | A solution for depleted local resources. |
| World map expeditions | Access to sites outside the local map. |
| Legacy worlds | A dead colony becomes a site for the next run (Q-005). |
| More traits | More differences between colonists in work and mood. |
| Religion from sites | Colonists honor old sites and legends. Shrines give mood. It uses the site system. |
| Social conflict | It comes from traits, mood, and scarcity. A few colony rules (rationing, curfew). |
| Diplomacy and allies | Stances and a few actions (gift, tribute, pact). Old grudges become manageable. |
| Raids on other settlements | Uses the expedition and combat systems. |
| More metals and longer chains | Longer goals with new knowledge. |
| Cave-ins, floods, plague, crop blight, thieves | More crises from the catalog (§10.1). |
| Deep underground layer | New content below the map (Q-002). |
| Social classes | Important above approximately 100 colonists. Uses housing and job type. |
| Playable fantasy races | Races exist as historical civilizations from the MVP. Playable races need balance work. |

### 16.3 Excluded systems (D-023)

This list is canonical. Do not implement these systems:

- Law as a separate system.
- Full z-levels.
- Fluid simulation.
- Tile-to-tile heat simulation.
- Simulation of gods.
- Detailed body anatomy in combat.
- Readable books with more text than short legends and chronicle entries.
- Family trees for historical figures.
- Spherical or wrapped worlds.
- Multithreaded generation.

## 17. Design risks

| Risk | Control |
|---|---|
| Scope creep | Obey the phase gates in `ROADMAP.md`. Put new ideas in the backlog. |
| History is only text | Obey R-040. In each playtest, record if a site changed a decision. |
| The player does not understand failure | Why chains, alerts, and the chronicle are part of each sprint, not polish. |
| Failure is unfair or boring | A gentle first winter. Notice scaling. Visible threats. |
| No long-term goals | Legends that point to far rewards. Sealed sites. Knowledge unlocks. |
| Too much art work | 16 × 16 tiles. A limited palette. Sprites tinted by material color. |
