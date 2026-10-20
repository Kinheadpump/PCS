# Infinite World Study (S11a, Q-016)

Status: Draft for the owner decision. This study is a design spike. It adds no feature to the game.

Writing rule: This document uses ASD-STE 100 Simplified Technical English. Code names are written as `code`.

## 1. Question and answer

The owner wants a technically infinite world. The world is now 256 × 256 world cells (D-019).

**Recommendation: no-go for the MVP. Go for a "realm" design in Phase 4.**

- An infinite world made of chunks works for height, temperature, and moisture. The prototype in `crates/worldgen/src/chunk.rs` shows this.
- Rivers, lakes, regions, deposits, and history read the whole world today. A chunk version of each one is a redesign and not a small change. It breaks every golden hash of world steps 5 to 12.
- The realm design keeps the finite generator as it is. The infinite world is a plane of realms. Each realm is one 256 × 256 world with its own sub-seed. It costs little and it keeps all existing tests.

The owner decides. Section 8 lists what the owner gets with each choice.

## 2. Terms

| Term | Meaning |
|---|---|
| Chunk | A square of 64 × 64 world cells. A chunk is a window on a function of the seed and the position. |
| Realm | One finite world of 256 × 256 world cells. Realm `(rx, ry)` has the sub-seed `mix(seed, rx, ry)`. |
| Pure layer | A layer that a function of seed and cell position gives. It needs no other cell. |
| Global layer | A layer that needs many cells to make one value: sea level, rivers, lakes, regions, history. |

## 3. What the prototype shows

The prototype makes the pure layers height, temperature, and moisture per chunk. It has these tests:

- A chunk is the same each time and it depends on the seed.
- Each chunk cell equals the direct cell function. So chunk borders have no seam, and the order of chunk generation does not matter.
- Chunks at negative positions and at positions far away (up to 20,000 chunks) work. The land share of a block of 16 chunks stays between 24% and 57%.
- The latitude repeats every 512 cells. The climate has no pole edge. This is not a wrapped world (`GAME_DESIGN.md` §16.3). The player never goes from the east edge to the west edge.

Speed: the tests make 144 chunks in less than 1 second in the dev profile. This is about 5 ms for a chunk of 4,096 cells. This is a rough value from a test run. It is not a P-nn benchmark. A map view of 5 × 5 chunks costs about 125 ms for the pure layers. A cache of chunks is necessary.

The prototype leaves out the global layers. The next sections say what each one needs.

## 4. Each world layer

### 4.1 Sea level

The finite world finds the sea level from the land share (D-019). A world without an edge cannot do this. The prototype uses a fixed sea level, the 60th percentile of the height of a large sample. Result: the land share changes by place from 24% to 57% in blocks of 256 cells. This is natural and it gives large oceans and large continents. The parameter `land_per_mille` changes meaning. It sets the fixed level, not the exact share.

### 4.2 Moisture and rain shadow

The finite world carries moisture along the wind from the ocean. It needs the cells upwind. The prototype uses the latitude band and noise, so it has no rain shadow. A chunk version can walk upwind for a fixed fetch of 64 cells. This costs 64 times more per cell unless a chunk keeps a halo of upwind cells. Plan: a halo of one chunk to the upwind side.

### 4.3 Rivers and lakes

Rivers need flow accumulation. A cell needs to know all cells that drain into it. Lakes need depression filling over a whole basin. Neither one works in a chunk.

Options:

1. **Realms.** Rivers stay as they are in each realm. A realm edge is ocean, so no river crosses it. Cost: none.
2. **Coarse drainage.** A river network on a coarse grid (one node for each 16 × 16 cells) with drain directions from a pure function. Chunks draw the river as a path between nodes. Lakes are separate noise-based basins. Cost: new algorithm, new golden hashes, and rivers look different from today.

### 4.4 Regions, geology, and deposits

Regions are connected areas with a minimum of one deposit type each (S9). A region must be a bounded set. Province points and rock layers are pure layers with a lattice of points, so chunk versions are easy. The region graph and the deposit repair step need the whole region. They work in a realm. They do not work on a plane without edges unless the regions have a maximum size. Plan for the chunk version: a region is the set of cells of one lattice cell of 32 × 32 cells that have the same biome group, and the repair step works inside it.

### 4.5 History

History simulates civilizations over 3,000 years on the whole world (D-019). It reads and writes the owner, road, and depletion grids.

- **Realms:** each realm has its own history. Civilizations do not cross a realm edge. Cost: none. A later step can add trade between neighbor realms.
- **Chunks:** history needs a different model. The seeds of the civilizations come from a lattice. A civilization has a maximum reach. The player sees one cell, so the game simulates only the cells in reach of that cell plus a halo. The result of a cell must not change when the player looks at it from another place. This is the hard rule: history of a cell is a function of the seed and the position only. This needs a new design and a long test effort.

### 4.6 Embark and the local map

`embark_from_world` reads the world cell and its neighbors up to a small distance. `generate_local_with_sites` needs the sites of the cell. Both work with realms with no change, and also with chunks if the neighbor read crosses a chunk border (it needs a halo of one chunk).

### 4.7 Save, hashes, and performance

- **Save:** a save stores the seed, the content hash, and the colony. It does not store the world. A chunk or a realm is generated from the seed again. This needs the generator to stay deterministic. A world step change breaks old saves. So the save needs a world generator version. The save does not have this field today.
- **Golden hashes:** realms keep all hashes, because realm `(0, 0)` can use the seed itself. New goldens are only for the plane layout. Chunks need new goldens for every layer.
- **P-05:** the 300 ms target covers one realm (about 61 ms for steps 1 to 12 today). The player waits for one realm at the start. Neighbor realms generate in the background of the world map. This conflicts with the rule "no background work" (`TECHNICAL_DESIGN.md` §1), so the game does it between frames, one realm at a time, in the main thread. Chunks cost 125 ms for a 5 × 5 view of pure layers, and the global layers add more.

## 5. Zoom and the world map

Today there are two levels: the local map (256 × 256 tiles) and the world map (256 × 256 world cells).

With realms:

1. Local map.
2. Realm map: the world map of today, for one realm.
3. Plane map: realms as large cells. Each shows its biome mix and its known sites. The map draws only realms that exist in memory or in the cache.

With chunks:

1. Local map.
2. Chunk view: a window of world cells around the camera.
3. Overview: a coarse level of the pure layers (one value for each 8 × 8 cells), cached.

The UI code from S9 (layers, inspector, seed box) works for a realm with no change. Level 3 is new work in both designs.

## 6. Gameplay effects

- The sim reads only the local map. It does not change.
- Rule 9 and the sites (R-041) work with realms. They need the new history model with chunks.
- Expeditions and trade (Phase 4) fit realms well: the player travels to a neighbor realm, and the cost is the distance. With chunks the same is possible, but the history of the destination depends on the new model.
- The player has no reason to need a world edge. The wish "technically infinite" holds in both designs: the realm plane has no limit.

## 7. Options

| Option | Work | Risk | Result |
|---|---|---|---|
| A. Finite world only | 0 | None | 256 × 256 world. The owner wish is not met. |
| B. Realms (recommended) | Small: sub-seed function, plane map, realm cache, save field. About 1 sprint. | Low | Infinite plane. All existing generator tests stay. Edges between realms are visible as ocean. |
| C. Chunks | Large: new sea level, moisture fetch, coarse rivers, region and history redesign. 4 to 6 sprints. | High | No visible edges. All golden hashes of world steps change. History is the biggest risk. |

An ocean edge between realms is a visible limit. The design can use it: the ocean is a strait, and sea travel crosses it. If the owner wants land across realm edges, then option C is the way. A mixed option exists: option B first, and option C later if the owner wants it. The realm sub-seed function and the cache are not wasted in option C.

## 8. Decision for the owner

1. Is the infinite world in the MVP? **Recommendation: no.** The MVP has 256 × 256 world cells.
2. When the owner wants the infinite world after the MVP, does an ocean edge between realms satisfy the wish? **Recommendation: yes, for Phase 4.** If no, plan option C after M3 and do a second spike on rivers and history.
3. What stays for now: world steps 5 to 12 do not change. The prototype in `chunk.rs` stays as a spike and the game does not use it. The file can be deleted when the owner picks option B.

## 9. Open points

- The speed value in §3 comes from a test run. A benchmark must measure it before any decision on chunk size.
- The land share of 24% to 57% per block needs a check with 20 seeds before option C starts.
- The latitude period of 512 cells is twice the size of the finite world (256 cells pole to pole). Climate bands are wider on the plane. A parameter can change this.
