# Art Direction

This document is the source of truth for the look of the game. Follow it for each new sprite. The method comes from `docs/topdown_pixel_art_agent_guide.md`. Decision D-034 requires that code makes all art. Do not draw by hand. Do not download assets.

## 1. Visual identity

The game is a colony simulation. The player watches many small people on a large map. The art has three goals.

1. **Units come first.** The player must find each colonist at once. Colonists have the highest contrast on the map. Ground and plants are quiet and mid-value.
2. **Material tells the truth.** Grass, soil, sand, clay, water, stone, ore, and trees look different in shape and in value, not only in hue. A player must know what a tile is at zoom 1.
3. **Calm, clean pixels.** Use groups of pixels with a plan. Do not use random noise as texture. A tile has at most two value steps of base noise plus a few planned marks.

Do not use: pillow shading, single pixels of random color, black outlines on ground, or texture that is as loud as a unit.

## 2. Camera and perspective

- The view is fixed top-down with a small front tilt (three-quarter top-down).
- Ground tiles show their top face only.
- Rock walls show the top face, and a front face on the south edge when the tile to the south is open.
- Colonists and trees show the front. A colonist shows the head above the body. Feet are on the lower part of the sprite.
- A colonist faces south when it stands. A walking colonist faces its next tile. East and west use one sprite and a flip. North and south have their own sprites.
- Objects that stand on the ground sort by their feet. Draw the object with the lower feet row last.
- A shadow of a tall object falls to the south east.

## 3. Pixel scale and grid

- A tile is 16 x 16 pixels. All sprites use cells of 16 x 16 pixels in a sheet of 16 columns.
- One sprite pixel is one square. The game draws the map at 16 pixels per tile in textures. The game draws sprites with nearest-neighbor filtering.
- Normal view is 32 screen pixels per tile (2 x). Supported integer views are 16, 32, 48, 64 screen pixels per tile. Between these values the pixels may differ by one screen pixel. Prefer integer values.
- Below 16 screen pixels per tile, map textures use linear filtering. Colonists are drawn at least 12 screen pixels high so that they stay visible.
- A colonist uses about 8 x 13 sprite pixels. The cell origin is the tile center. The feet are on row 13. The ground shadow is on row 14.
- A tree uses about 13 x 13 sprite pixels and leaves a gap of 1 pixel to 3 pixels at the cell edge.
- A larger object uses a whole number of tiles. Keep the same pixel size and the same outline style.

## 4. Palette and light

The game tints grayscale sprites with the material color from `assets/content/materials.ron`. The value 224 is the plain material color. A lower value is darker. A higher value is lighter.

| Role | Gray value | Use |
|---|---|---|
| Outline of trees and ore | 60 to 90 | Dark edge of an object on the ground |
| Deep shade | 150 to 170 | Seams, cracks, lower right of forms |
| Base | 215 to 230 | Main area of a material |
| Light | 240 to 255 | Upper left of forms, tufts, highlights |

Unit colors do not use the tint system, with one exception: the shirt. Unit colors are fixed.

| Part | Color (red, green, blue) |
|---|---|
| Outline | 30, 24, 40 (dark purple, not black) |
| Hair, hair light | 88, 58, 40 and 128, 88, 56 |
| Skin, skin shade | 236, 188, 148 and 200, 144, 112 |
| Eye | 40, 30, 44 |
| Pants, pants shade | 72, 80, 112 and 50, 54, 82 |
| Boots | 92, 62, 44 |
| Shirt | Gray 150, 224, 255. The game multiplies it with the colonist color. |
| Ground shadow | Black with 35 percent alpha |

Rules:

- The light comes from the north west. Light sides face north and west. Dark sides face south and east.
- Shadows shift cooler (more blue) when the tint allows it. A shadow edge on the map uses a flat dark color with partial alpha.
- No non-shirt pixel of a unit sprite may be gray. The test `only_the_shirt_is_gray` checks this.
- The selection mark is yellow (255, 242, 51). Yellow is not used for any other game object.
- Check each new sprite in grayscale. The silhouette and the value steps must still read.

## 5. Shape language

- Units: small, upright, round head, clear shoulders. The head is 6 pixels wide. The body is 8 pixels wide with arms.
- Plants: round lumps with a dark outline. A tree is a group of five round clumps. Each clump is lighter at the north west and darker at the south east.
- Rock: flat plates with a bevel. Each plate has a dark seam at the top and the left, a light line under the seam, and a dark bottom line.
- Water: flat, with short light waves. Foam is light and sits at the shore.
- Ground: flat. Marks are small and sparse.

## 6. Outlines and clusters

- Units have a 1 pixel dark outline on all sides except the shadow.
- Trees have a 1 pixel dark outline. The lit side of the outline is 14 steps lighter.
- Ground tiles have no outline.
- Avoid single pixels that do not belong to a form. A mark is at least two pixels, or one root pixel with a blade above it.
- Do not use dithering. Do not use anti-aliasing in sprites.

## 7. Materials

| Material | Rule |
|---|---|
| Grass | Base value 222 plus or minus 3. Five tufts per tile. A tuft is one dark root pixel, a light blade above it, and a light blade to the side. |
| Bare soil | Darker base with dark clods. A patch has a grass fringe at its edge. |
| Sand | Light base with short ripples (light line over dark line). |
| Clay | Base with thin dark cracks. |
| Water | Dark base. Short light waves. Light foam at the shore. |
| Stone floor | Base with one crack and a few light chips. |
| Rock wall | Staggered plates (see section 5). Light edge on north and west open sides. Dark edge on east open side. A front face on the south open side. |
| Ore | Two clusters of 4 x 4 pixels with a dark rim. Ore is lighter than the rock around it. |
| Trees | See section 5. Color is a dark green made from the grass color. |

## 8. Terrain transitions

- Each ground type has a rank: stone 0, sand 1, clay 2, bare soil 3, grass 4. The tile with the higher rank grows a ragged fringe over the tile with the lower rank. The fringe uses the edge masks of the sprite sheet.
- Water has foam on the water tile. The land next to water is darkened.
- A wall casts a shadow on the open tile north of it and west of it.
- The map has no transition for diagonal neighbors. This is a known limit.

## 9. Animation

- Each facing has 3 sprites: stand, step 1, step 2.
- The walk cycle is step 1, stand, step 2, stand. A cycle takes 1 tile of distance. The game chooses the frame from the distance walked, so the feet do not slide.
- The ground point stays on row 13 in all frames.
- A standing colonist uses the south stand sprite.

## 10. Files

| File | Content |
|---|---|
| `crates/tools/src/assetgen.rs` | Ground tiles, trees, walls, edge masks, PNG writer, preview sheet |
| `crates/tools/src/assetgen_actors.rs` | Colonist sprites, written as text rows |
| `assets/sprites/tiles.png` | Grayscale tile sheet (256 x 48) |
| `assets/sprites/actors.png` | Full color colonist sheet (256 x 16) |
| `assets/sprites/preview.png` | Contact sheet for people. The game does not load it. |
| `crates/game/src/sprites.rs` | Sheet loading and tinting |
| `crates/game/src/render.rs` | Map baking, transitions |
| `crates/game/src/actors.rs` | Colonist drawing |

Run `cargo run -p tools -- assetgen` to make all files again. The tests check that the shipped files equal the generator output.

To add a unit sprite: add its text rows in `assetgen_actors.rs`. Use only the listed symbols. Add a cell number to `sprites.rs`. Keep the feet on row 13.

## 11. Checks before you accept a sprite

1. View it at 2 x and at 4 x in the game (`PCS_SHOT`, `PCS_ZOOM=32` and `PCS_ZOOM=64`).
2. View it in a group on each ground type: grass, sand, stone, and next to trees.
3. Check the silhouette. Fill it with one color. The shape must be clear.
4. Check the grayscale value steps.
5. Run `cargo test --workspace`.

## 12. Known weak points

- The ground has no diagonal transitions.
- Trees use 2 variants. A large forest shows repeats.
- Water has no animation.
- Colonists have one body type. All colonists have the same hair and skin. Only the shirt differs.
- There are no sprites yet for items, buildings, animals, or effects.
- A standing colonist always faces south. The sim stores no facing.
