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

- Each facing has 3 walk sprites: stand, step 1, step 2. The walk cycle is step 1, stand, step 2, stand. A cycle takes 1 tile of distance, so the feet do not slide.
- Each facing has a work pose without a tool: both hands go up and down, and the body moves 1 pixel. The game alternates the two phases from the job progress.
- A working colonist faces the job target. A walking colonist faces the move direction. An idle colonist faces south.
- The ground point stays on row 13 in all frames.

## 9a. Buildings

- `buildings.png` is a gray sheet. The game tints each pixel by its alpha value: 255 material color, 254 cloth, 253 fire, 252 metal. A lower alpha is a black shadow.
- A wall has a top (cap) and a front face of 5 rows. The face shows only below a wall that has no wall to the south. A wall cell has 16 variants: one for each set of connected sides. Doors join walls in the same way.
- Stone walls show courses of bricks. Wood walls show vertical planks. A door shows two posts and a leaf. The door runs east to west if a wall is to its east or west.
- A floor has no outline. A stone floor has flagstones. A wood floor has boards.
- A roof is a tint over the tiles below it (alpha 0.30). It shows courses of shingles and an outline on the free edges only.
- A bed has a cloth blanket. A hearth has a metal ring and 2 flame frames that change every 14 ticks. A workshop has a bench with a metal tool.
- A blueprint is a ghost of the finished sprite: alpha 0.55, a light blue tint, and a blue frame. A bar at the bottom shows the progress at zoom 8 and above.
- A build preview shows the ghost with a blue fill and a blue frame. A tile where the building cannot go gets a red fill.
- The rooms overlay tints each closed room with one of 6 colors. None of the colors is yellow, because yellow marks the selection.
- Light comes from the north west. The sprites have no dithering and no anti-aliasing.

## 10. Files

| File | Content |
|---|---|
| `crates/tools/src/assetgen.rs` | Image type, PNG writer, `make_all`, tests |
| `crates/tools/src/assetgen_terrain.rs` | Seamless ground textures (128 x 128 each) and ore sprites |
| `crates/tools/src/assetgen_objects.rs` | Trees, wall pieces, other objects |
| `crates/tools/src/assetgen_items.rs` | Item sprites and interface icons (with the Build and Remove icons) |
| `crates/tools/src/assetgen_buildings.rs` | Wall, door, roof, floor, bed, hearth, and workshop sprites |
| `crates/tools/src/assetgen_actors.rs` | Colonist sprites, written as text rows, 4 hair styles |
| `crates/tools/src/assetgen_preview.rs` | Palette sheet, reference sheet, repeat sheet |
| `crates/game/src/palette.rs`, `cells.rs` | Palette and sheet cell numbers. The tools include these files. |
| `crates/game/src/bake.rs` | Pixel bake of each chunk: fringes, shore, wall faces, shadows, decor, trees |
| `crates/game/src/actors.rs`, `overlay.rs`, `icons.rs` | Colonists, items and markers, icons |
| `crates/game/src/structures.rs` | Building textures, blueprint ghosts, build preview, rooms overlay |
| `crates/game/src/ui.rs`, `hud.rs`, `layout.rs`, `build_ui.rs` | Framed panels, icon buttons, text with shadow, panel placement, build panel |
| `assets/sprites/*.png` | Generated sheets. `preview.png` is a contact sheet that the game does not load. |

Run `cargo run -p tools -- assetgen` to make all files again. The tests check that the shipped files equal the generator output.

## 11. Checks before you accept a sprite

1. View it at 2 x and at 4 x in the game (`PCS_SHOT`, `PCS_ZOOM=32` and `PCS_ZOOM=64`).
2. View it in a group on each ground type: grass, sand, stone, and next to trees.
3. Check the silhouette. Fill it with one color. The shape must be clear.
4. Check the grayscale value steps.
5. Check that each ground texture repeats without a seam (the tests do this).
6. Run `cargo test --workspace`.

## 12. Known weak points

- Water has no animation.
- Colonists have one body type. Hair style, hair color, skin, and shirt differ.
- Animals have no sprites yet.
- Wood walls and wood floors have a similar color. A wood wall is darker and has vertical planks, but it is hard to see under a roof tint.
- The door is small at low zoom.
- Rock plates and clay cracks are still busy at close zoom.
- At 1280 x 720 the panels fit only if some are left out. The help panel and the work panel give way first. A test checks that no panels overlap.
- The ground texture of a class repeats every 8 tiles. A very large field of one class can show the repeat.
