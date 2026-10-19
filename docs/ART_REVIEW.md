# Art Review

This document records the state of the art against `docs/topdown_pixel_art_agent_guide.md` (the guide). It has three parts: the audit of the first pass, the work of the second pass, and the open defects.

Status words: **done** (the evidence proves it), **partly** (some of it exists), **skipped** (nothing exists).

## 1. Audit of the first pass (commit 9b1b54d)

The first pass added `docs/ART_DIRECTION.md`, nine colonist sprites, and rewrote the grass, tree, wall, and edge sprites. It did not follow the guide as a whole. The owner is right that the result looks rudimentary. The first pass made assets first and did the review work only in part.

### 1.1 Stages of guide section 11

| Stage | Status | Evidence | Gap |
|---|---|---|---|
| 1 Audit | partly | `ART_DIRECTION.md` section 12 lists weak points. Scratch screenshots of the three presets existed (not committed). | No audit report with priorities. No baseline image in the repository. No list of what already worked. |
| 2 Art direction | partly | `docs/ART_DIRECTION.md` exists with camera, scale, value table, shape rules. | No palette file and no swatch sheet. Only gray value bands, so no hue-shifted ramps. No visual reference sheet. The document contradicts the game (see 1.3). |
| 3 Reference assets | skipped | `assetgen.rs` made all ground, wall, edge, tree, and colonist sprites in one step. | No small reference set. No review of a reference set before the rest. `preview.png` shows people only. |
| 4 Representative scene | partly | In-game screenshots of the presets at several zoom levels were taken during the work. | No defined scene with stockpile, items, designations, working colonists, and panels. Items and UI were not in the scene. No scene test is in the repository. |
| 5 Review and correction | partly | The commit says "cleaner tiles". | One correction pass. No written review. No update of the art bible after the review. The main defects stayed (see 1.2). |
| 6 Controlled production | partly | Tiles and colonists only. | Items stayed as text patterns in `overlay.rs` (D-038). No sprite for tools, designations, zones, or icons. The UI was not touched. |
| 7 Final validation | partly | `cargo test` passed. Hash tests and file-equals-generator tests exist. | The checks of guide section 12 were not run as a list. No greyscale view, no silhouette view, no repeat grid. |

### 1.2 Quality checks of guide section 12 (first pass)

| Check | Status | Evidence |
|---|---|---|
| Pixels have the intended shape | partly | Ground tiles use random noise on every pixel (`fill_noise` in `assetgen.rs`). The guide calls this noise. |
| No soft edges or anti-aliasing | partly | The edge masks use random alpha ramps. The tree shadow has alpha 80. Both blend colors. The art direction says "no anti-aliasing". |
| Palette follows shared rules | skipped | There is no shared palette. Shadows only get darker; they do not shift hue. |
| Shadows use one light model | skipped | Light comes from the north west, but walls shade the tiles north and west of them (`render.rs`). |
| Outlines consistent | partly | Colonists and trees have outlines. Items have their own outline values. Ground has none. |
| Objects of different size belong together | partly | Colonist, tree, and item sprites use different shading rules and different detail density. |
| Player or main unit easy to see | partly | Colonists are bright. A colonist and a tree have the same size on screen at zoom 16. Not tested in greyscale. |
| Gameplay elements distinct from decoration | partly | Items and markers are readable. Ore is a set of small dots that are hard to find in a rock field. |
| Readable in greyscale | skipped | Not tested. |
| All edges and corners exist | skipped | Only four straight edge masks. No diagonal or corner pieces. The art direction calls this "a known limit". The guide asks for corners. |
| Tiles work in a grid | skipped | Not tested. A brightness step for each 4 x 4 tiles makes square blocks that show in every shot. Soil patches are hard 2 x 2 squares. |
| Repeats do not distract | skipped | Four tile variants and two tree shapes. A forest looks like a stamped grid of one tree. |
| Animation frames and ground point | partly | Test `feet_stay_on_one_row_in_each_standing_frame` exists. There are no idle, work, or carry frames. A standing colonist always faces south. |
| Game loads assets | done | `shipped_*_matches_generator` tests and the sheet decode tests. |
| Filtering and scaling | partly | Nearest filter is used. The art direction says the normal zoom is 32 pixels per tile, but `START_ZOOM` is 16. |

### 1.3 Other defects found in the audit

- The art direction says the normal view is 32 pixels per tile. The game starts at 16.
- All colonists have the same hair and skin. Only the shirt differs.
- Water has one flat tone. The shore is a blurred gradient.
- Rock walls read as a brick wall. Large rock fields are one flat texture.
- Items, the stack count, the designation markers, and the zone are drawn in `overlay.rs` with rectangles. The icons are 5 x 5 pixels.
- The interface uses flat translucent rectangles with a 1 pixel border. Buttons have no icons.

### 1.4 Definition of done (guide section 15) after the first pass

| Item | Status |
|---|---|
| Current art audited | partly |
| Art direction documented | partly |
| Palette and reference assets available | skipped |
| Representative scene shows the style works | partly |
| Main visual defects corrected | skipped |
| New assets follow the art direction | partly |
| Terrain transitions and repeats tested | skipped |
| Assets load and render | done |
| Checks run | partly |
| Report lists changes, results, remaining work | partly |

## 2. Second pass

See section 3 of this document, written at the end of the second pass.

## 3. Second pass results

Screenshots with the same views before and after are in `docs/shots/` (`before_N.png`, `after_N.png`, N = 2, 4, 5, 6).

### 3.1 What changed

- Ground: seamless gray textures per ground class, sampled by world pixel. There is no tile grid. Edges between classes are ragged fringes, also on diagonals. Water has foam and depth bands. Walls have a lit top, a south face, and shadows.
- Trees: variants with offsets and species patches, and a shadow to the south east.
- Colonists: 4 hair styles, hair, skin, and shirt colors, facing, and a work pose without a tool.
- Items and icons: drawn in `assetgen_items.rs`. Logs, rocks, ore, ingots, and lumps have a tier and a count.
- Interface: framed panels with a bevel, text shadow, tool icons on buttons, designation brackets and badges, dashed zone border.
- A test bug was found and fixed: the soil and sand textures used a noise spacing that does not divide 128, so they had a seam when repeated. The seam test now compares with the largest inside step.

### 3.2 Golden hash

The test `sheet_png_known_answer` was removed because every sprite changed on purpose. The test `shipped_files_match_the_generator` now guards the output. The sim golden hash did not change from the art work.

### 3.3 Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all passed at the end of this pass.

### 3.4 Open defects

- The colonist panel and the toolbar overlap in a 1280 x 720 window.
- Work panel column headers use 3 letters.
- Rock and clay textures are busy at high zoom.
- No animation for water.
- `repeat.png` is large (about 4.7 MB).
