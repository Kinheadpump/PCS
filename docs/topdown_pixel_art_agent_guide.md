# Top-Down Pixel Art: Agent Guide and Production Standard

## 1. Purpose

Use this guide to improve pixel art for any top-down game. It applies to games in many genres, including action, adventure, role-playing, tactics, simulation, farming, survival, horror, strategy, and puzzle games.

The goal is to make the art clear, polished, coherent, technically correct, and suitable for play. The goal is not to add detail to every image. The goal is to create a strong visual system and apply it to every asset.

Do not assume that all top-down games need the same art style. First study the game. Then choose visual rules that fit its purpose, camera, mood, gameplay, and technical limits.

## 2. Role and main instruction

Act as the lead pixel artist, art director, and technical-art developer for the game you inspect.

Do not only give advice. Inspect the available project, find the main visual defects, define the art direction, create or improve assets, integrate them, and test the result in the actual game.

Use the following order:

1. Understand the game and its current art.
2. Define the visual direction and production rules.
3. Create a small reference set.
4. Build and test a representative scene.
5. Improve the art based on the test.
6. Expand the asset set in controlled groups.
7. Validate the art in the game and document the results.

Do not make a large number of assets before the visual direction is clear.

## 3. First, inspect the game

Read the project documentation and inspect the relevant code, art files, scenes, maps, and rendering settings. Use the actual project as the main source of truth.

Identify and record:

- The game genre, player actions, intended mood, and target audience.
- The camera type. For example, it may be true top-down, three-quarter top-down, or another fixed top-down view.
- The camera angle and projection rules.
- The internal resolution and target display sizes.
- The tile grid, common sprite sizes, and intended pixel scale.
- The texture filtering, scaling, camera movement, and zoom rules.
- The drawing order and rules for objects that pass in front of or behind other objects.
- The current ground, terrain, props, characters, structures, effects, interface, and animation assets.
- The file formats, transparency rules, asset naming, and import settings.
- The method used to build maps and place or vary repeated assets.
- The gameplay meaning of visual elements. Identify walkable ground, solid objects, hazards, resources, targets, and decoration.

Find the main reasons why the current art looks weak. Check for inconsistent pixel size, unclear shape, poor contrast, too much noise, weak palette control, mixed perspectives, uneven detail, repeated patterns, broken terrain edges, poor lighting, or incorrect in-game scaling.

Record which current assets work well. Reuse them when they fit the new rules. Do not replace a working system without a clear reason.

Do not assume an engine, resolution, tile size, palette size, or camera angle. Find these values in the project. If the game does not define a value, choose one through a small test and document the choice.

## 4. Define the visual direction

Create or update a document named `ART_DIRECTION.md` in a suitable art or documentation folder. Use the project's current folder structure when one exists.

This document is the main source of truth for the art style. It must give clear rules that another artist or agent can follow. Avoid vague rules such as “make it beautiful,” “make it cozy,” or “add more detail.” Explain what these words mean in visible terms.

### 4.1 Visual identity

Write a short description of the intended visual identity. Define two to four main visual goals. Each goal must guide real design choices.

For each goal, state:

- What the player should see or feel.
- Which shapes, colors, light, texture, and composition support that goal.
- Which visual choices do not fit that goal.

Do not force a genre or mood that the game does not use. The same pixel-art principles can support very different game identities.

### 4.2 Camera and perspective

Define what the camera can see and how objects are drawn.

State:

- Whether the view is true top-down, three-quarter top-down, or another fixed view.
- Which top and side surfaces are visible.
- How roofs, walls, floors, trees, cliffs, units, and objects show their height.
- How the ground position of a character or object is shown.
- How overlapping objects are sorted.
- How the view treats tall objects, shadows, and objects that can hide the player.

Apply the same camera logic to every asset. Do not mix a true overhead tree with a side-view tree unless the difference is intentional and documented.

### 4.3 Pixel scale and grid

Define:

- The base tile size or tile sizes.
- The usual canvas sizes for each asset class.
- The intended pixel size at the normal in-game view.
- Rules for sprites that are larger than one tile.
- Rules for sprite origins, feet positions, pivots, and alignment.
- The required scaling and filtering method.

Use pixel dimensions that fit the game. Do not copy a fixed size from another project without testing it.

Keep the pixel scale coherent. A small character should not look as if it came from a different game than a large tree or building. Larger objects may need more pixels, but their edge style, cluster style, and detail density must still match.

### 4.4 Palette and color ramps

Create a shared palette file or another reliable color reference. Record exact color values and the purpose of each color.

Define the palette rules:

- Main colors for the game world.
- Colors for important gameplay elements.
- Colors for shadows and highlights.
- Colors for outlines, if outlines are used.
- Material ramps for common materials.
- Accent colors and when to use them.
- Rules for dark scenes, bright scenes, weather, damage, magic, or other special effects where present.

Build color ramps with clear value steps. Use hue shifts when they improve the style. For example, a shadow may shift toward a cooler or warmer hue instead of only becoming darker. Test the result in the full scene.

Use color to support shape and gameplay. Important objects must not disappear into nearby ground. Do not give every asset the highest contrast. Reserve strong contrast and bright accents for elements that need attention.

A limited palette is often a useful starting point, but no single color count fits every game. Choose a palette size that supports the desired style. Reuse colors when possible. Add a new color only when it has a clear purpose.

Check both color and value. If important shapes become hard to read in greyscale, revise their light-dark separation.

### 4.5 Shape language and silhouettes

Define the main shape language. State whether forms should look sharp, round, blocky, organic, clean, rough, heavy, light, or another clear style.

Create clear shape rules for characters, creatures, plants, rocks, buildings, props, and other important asset groups.

Use this drawing order for most sprites:

1. Block in the full silhouette.
2. Check that the shape is recognisable without inner details.
3. Add the largest color regions.
4. Add the main planes, shadows, and highlights.
5. Add material texture and small details.
6. Remove details that do not help recognition, shape, mood, or gameplay.

Test sprites at their normal game size. Do not approve an asset only because it looks good when enlarged.

Important objects must remain recognisable against the backgrounds where they appear. Use a silhouette test when needed: fill the object with one solid color and check its shape.

### 4.6 Pixel clusters and edges

Pixel art is not just a large image that looks blocky. Pixel placement must be intentional.

Use groups of pixels to describe shapes, planes, materials, and light. Avoid noise that does not explain the form.

Inspect edges and clusters. Correct:

- Unplanned single-pixel dots.
- Broken or uneven outlines.
- Accidental gaps.
- Jagged edges that do not fit the shape.
- Long, parallel pixel bands that create unwanted visual patterns.
- Clusters that look like unrelated blocks.
- Excessive checker patterns.
- Blurry or soft edges that break the pixel style.

Use dithering only when it supports the selected style or creates a useful transition. Do not use it to hide weak shading. Use anti-aliasing only when it matches the style and uses deliberate colors. Do not use automatic smoothing without checking the result at normal size.

Avoid pillow shading. Do not place a bright area in the centre and a dark border around the full object unless the material and light truly require it. Shade according to the form and the defined light direction.

### 4.7 Lighting and shadows

Define one default light direction unless the game's art style requires a different model. State how highlights, cast shadows, contact shadows, and ambient darkness work.

Apply the same logic to every asset. The light direction may change in a special scene, but that change must be intentional and visible in the scene rules.

Shadows must support form and position. Do not allow them to hide gameplay information or make the object appear to float. Make sure the shadow meets the ground near the object. Use light and shadow to separate layers and height when this fits the style.

If the game has day-night changes, weather, or dynamic lights, define how the base art reacts. Preserve readable values when the scene becomes dark.

### 4.8 Material rules

Define a separate visual method for each common material. Describe its shape, palette, texture, edge treatment, and highlights.

Possible materials include:

- Grass and other ground cover.
- Soil, mud, sand, snow, and ash.
- Stone, rock, cliffs, and paving.
- Wood, bark, leaves, and other plants.
- Water, ice, and wet surfaces.
- Metal, glass, fabric, leather, and masonry.
- Fire, smoke, magic, and other effects.

Only include materials that the game needs. Do not apply the same noisy texture to every material. Texture must explain the material and remain readable at normal game size.

## 5. Design the whole scene, not only each sprite

A group of strong sprites can still produce a weak scene. Review the art in its environment.

Use a clear visual hierarchy. The player and important gameplay elements must be easy to find. Decoration must not hide the path, boundary, target, resource, hazard, or object that the player must use.

Control the density of detail. Use more detail where it helps recognition or creates a useful focal point. Leave quieter areas where the player must read movement or ground shape.

Use large, medium, and small shapes. Avoid scenes where every object has the same visual size, contrast, and texture density.

Use repeated objects with controlled variation. Variation may change shape, rotation, state, or detail when the asset and camera support it. Do not make variations so different that they break the art style.

Design landmarks and paths when they help the player navigate. Keep these choices suitable for the game genre. An arena, farm, tactics map, dungeon, and quiet exploration area may need different composition rules.

Check the scene from the normal camera position. Review the whole screen and the local detail around the player. A view that looks good as a large concept image may fail when the player moves through it.

## 6. Build terrain and tilesets as systems

Do not treat ground tiles as unrelated pictures. Treat a tileset as a system of pieces that must work together.

First inspect the game's map and tile system. Then define the tiles that it actually needs.

For each terrain type, consider:

- A base or interior tile.
- Edge and corner transitions.
- Connections to each neighbouring terrain type.
- Diagonal connections where the map system supports them.
- Narrow strips, small patches, and irregular boundaries.
- Decorative overlays that should not change the base terrain.
- Variation tiles that reduce visible repetition.
- Special tiles for slopes, cliffs, paths, shorelines, damage, or other terrain states where present.

Use the project's actual tile system. Do not create extra variants that the engine cannot use. Do not omit variants that the map system requires.

Keep tile boundaries clean. Avoid gaps, visible seams, inconsistent light, and textures that stop abruptly at the tile edge unless the break is intentional.

Keep decoration separate from base terrain when this fits the engine. This allows the game to vary details without breaking terrain connections.

Test tiles in a repeat grid, not only as single images. Test every edge and corner. Test larger arrangements and awkward shapes. Check for obvious repeated patterns, unintended lines, and visual noise.

Use a tile preview, map preview, or automated contact sheet when the tools support it. Test the output inside the game too.

## 7. Characters, creatures, objects, and structures

Apply the art direction to all asset groups.

For each important asset, define:

- The purpose of the asset in gameplay.
- Its typical in-game size.
- Its silhouette and key identifying features.
- Its palette and contrast level.
- Its material rules.
- Its ground position, pivot, and draw order.
- Any animation, damage state, interaction state, or visual variant that the game needs.

Keep characters readable against common ground types. Do not let important body parts disappear into the background. Use contrast with care. A clear silhouette often works better than many tiny details.

Make interactive objects visually distinct from decoration where the game requires this. Use consistent cues for object types and states. Do not rely only on color when shape or another cue can also help.

For structures, keep the view, roof shape, wall depth, door placement, and shadow logic consistent. If the camera shows side surfaces, use the same rule for their size and direction.

For small objects, do not spend many pixels on details that disappear at normal size. Keep the most important features clear.

## 8. Animation rules

Define animation rules before creating many animated assets.

For every animation, record:

- Frame size and frame count.
- Frame timing or timing method.
- The pivot or ground point.
- The direction set required by the game.
- The loop rule.
- The required idle, movement, attack, use, damage, or other states.
- How shadows and attached objects behave.

Use the direction set that the game requires. It may use one direction, four directions, eight directions, or another design. Do not make extra directions without a gameplay or presentation need.

Keep the character's ground point stable during movement unless the motion requires a real jump or height change. Avoid accidental changes in body size, outline thickness, palette, or pixel density between frames.

Check animation at normal speed and normal game scale. Look for shape changes, flicker, unintended sliding, uneven timing, and frames that break the silhouette. Review the animation in context, not only in a sprite editor.

Reuse frames only when this does not harm the motion. Use the animation tools available in the project. Keep source files editable where possible.

## 9. Use AI image generation with care

Image generation can help create concepts, draft sprites, or explore a visual direction. It does not guarantee production-ready pixel art or style consistency.

If you use an image model:

1. Provide the approved art bible and visual references.
2. Request one clear asset or a small related group at a time.
3. State the required view, canvas size, palette rules, silhouette, transparency, and output format.
4. Do not let the model invent a new camera, palette, outline style, or pixel density.
5. Inspect the result at its native pixel size.
6. Check for soft edges, inconsistent pixel sizes, stray colors, poor clusters, mixed perspective, and accidental background pixels.
7. Edit or redraw defects before integration.
8. Test the cleaned asset in the same scene as approved assets.

Treat generated output as a draft unless it passes all project checks. Do not place a large generated sheet into the game without inspecting each sprite. A sheet can contain different styles or errors in different regions.

Use a code-based or image-based workflow when it improves control. For example, scripts can check image dimensions, transparency, naming, and palette use. A script cannot replace all artistic judgment.

Do not copy another game's distinctive characters, maps, or assets. Use references to study general methods, composition, and visual goals. Create original assets for the game.

## 10. Technical asset and rendering rules

Follow the engine's actual requirements. Confirm these requirements before export or import.

Check:

- File format and colour mode.
- Canvas size and sprite-sheet layout.
- Transparency and alpha edges.
- Frame and tile boundaries.
- Asset names and folder locations.
- Import settings and compression.
- Texture filtering and scaling.
- Camera zoom and sub-pixel movement.
- Layer order and object sorting.
- Tile collision, object origins, and gameplay alignment.

Use lossless files for pixel-art source assets. PNG is a common choice for exported sprites. Keep editable source files when the tool supports them and the project benefits from them.

Nearest-neighbour filtering is often suitable for classic pixel art because it preserves hard pixel edges. Confirm the choice in the target engine and test it at the actual display scale. Some projects use intentional mixed rendering methods, but they must define and control those methods.

Do not scale sprites by arbitrary fractions if this produces uneven pixel sizes. Define supported scale factors and camera settings. Check scrolling and zoom in motion, not only in a still image.

## 11. A staged production workflow

### Stage 1: Audit

Inspect the game. List the most important art defects. Record the existing constraints and reusable assets. Capture a baseline image if possible.

**Output:** A short audit report with priorities.

### Stage 2: Art direction

Create `ART_DIRECTION.md`. Define the camera, pixel scale, palette, light, shapes, materials, detail levels, and asset rules. Create a shared palette and visual reference sheet when the tools support them.

**Output:** A written and visual standard.

### Stage 3: Reference assets

Create a small group of high-value assets. Include at least one typical ground tile, one terrain transition, one small prop, one large object, and one character or gameplay object when these asset types apply.

Use the group to check that the visual rules work across different shapes and sizes.

**Output:** A reference set that establishes the style.

### Stage 4: Representative scene

Build a small scene in the game. Include a player or main unit, typical ground, terrain transitions, props, and other essential visual systems. Use real rendering, sorting, and lighting.

**Output:** A working scene that tests the art direction.

### Stage 5: Review and correction

Inspect the scene at normal game scale. Find style mismatches, readability problems, repeated patterns, poor edges, and technical defects. Correct the art bible when a rule is weak. Fix the assets to follow the corrected rule.

Do not continue to large-scale production while the main visual defects remain.

**Output:** An approved visual standard and corrected reference set.

### Stage 6: Controlled asset production

Create assets in small groups. Work on foundational assets before rare decoration. After each group, compare the new assets with the reference set and test them in the game.

Maintain the art bible as the single source of truth. Record every intentional change to the style.

**Output:** Consistent and integrated asset groups.

### Stage 7: Final validation

Run the game. Test the scenes that use the new assets. Check the items in Section 12. Fix important defects before reporting completion.

**Output:** A tested result, a list of changed files, test results, and remaining work.

## 12. Quality checks

Do not mark a check as passed unless you have performed it.

### Pixel quality

- Pixels have the intended size and shape.
- Edges and clusters look deliberate.
- There are no stray pixels, accidental gaps, or unwanted soft edges.
- Dithering and anti-aliasing follow the style rules.
- Small details remain useful at normal game scale.

### Visual consistency

- The camera and perspective match across assets.
- Palette use follows the shared rules.
- Shadows and highlights use the correct light model.
- Outlines and edge treatment are consistent.
- Material texture and detail density fit the art direction.
- Objects of different sizes still look as though they belong to the same game.

### Readability and composition

- The player or main unit is easy to see.
- Important objects are easy to recognise.
- Gameplay elements are distinct from decoration when required.
- The art does not hide important paths, boundaries, hazards, targets, or interactions.
- The scene has a clear visual hierarchy.
- Important shapes remain readable in greyscale when value contrast is required.

### Terrain and repetition

- All required tile edges and corners exist.
- Adjacent terrain connects without unintended seams.
- Tiles work in a grid and in realistic map shapes.
- Repeated patterns do not distract from play.
- Random variation follows the same visual rules.

### Animation

- Frames keep the required size and alignment.
- The pivot or ground point is stable where required.
- The animation does not cause unintended flicker or shape changes.
- Timing and motion fit the game.
- The animation works in the real scene.

### Technical correctness

- The game loads the assets correctly.
- Texture filtering and scaling look correct.
- Transparency, sorting, and draw order work correctly.
- Collision and interaction positions match the visible art.
- Automated checks pass where they exist.
- The project builds and runs after the changes.

### Useful tests

Use tests that suit the project:

- Inspect assets at native size and normal game scale.
- Make a contact sheet of related assets.
- Repeat each tile in a large grid.
- Test transitions between every required terrain pair.
- View critical assets as solid silhouettes.
- Inspect a greyscale version for value problems.
- Compare assets side by side with the reference sheet.
- Play the scene and move the camera or character.
- Run automated checks for dimensions, naming, palette limits, alpha, and missing files where useful.

## 13. Project documents and maintenance

Keep the production rules in a known location. Use the existing project structure when possible.

The recommended documents are:

- `ART_DIRECTION.md` — the rules that define the style.
- `ASSET_PIPELINE.md` — how to create, edit, export, import, and validate art.
- `ART_REVIEW.md` — current defects, decisions, and unresolved risks.

Create only the documents that are useful for the project. Do not create empty or duplicate files.

Use clear asset names and folders. Keep the relationship between source art, exported assets, and in-game use easy to understand.

When a new asset type appears, use the existing rules first. Add a new rule only when the game needs it. Record why the rule changed and which assets must follow it.

A future agent must be able to create a new asset without inventing a new art style.

## 14. Working rules for the agent

- Inspect before changing.
- Use the project's actual files and behaviour as evidence.
- Do not guess values that you can inspect.
- Do not assume that all top-down games use the same camera or visual style.
- Do not replace the whole asset set before testing the art direction.
- Do not add detail only to make the image look complex.
- Prefer clear silhouettes, controlled colour, deliberate pixel clusters, and readable scenes.
- Do not rewrite unrelated gameplay or engine systems.
- Keep source assets and rules easy to edit.
- Make real changes. Do not stop after writing a plan.
- Test assets in the actual game. Do not judge them only in an image editor.
- Report the checks that you did and their results.
- State which checks you could not perform. Do not claim that untested work passed.
- Write project documents in ASD-STE 100-style simplified technical English. Use short sentences, common terms, active voice, and one instruction per sentence where possible.

## 15. Definition of done

The task is complete when:

- The current art has been audited.
- The art direction is documented with clear and usable rules.
- The palette and reference assets are available to the agent.
- A representative scene shows that the style works in the game.
- The main visual defects have been corrected.
- New or revised assets follow the art direction.
- Terrain transitions and repetitions have been tested where applicable.
- The assets load and render correctly in the target game.
- The project has passed the checks that were possible to run.
- The report lists the changes, real test results, and remaining work.

A large number of unreviewed sprites is not a successful result. A coherent and tested visual system is the successful result.

## 16. Further reading

Use these sources to study core techniques and tool behaviour. Use the game and its approved art direction to decide which techniques apply.

- [The Pixel Art Tutorial — Pixel Joint](https://pixeljoint.com/forum/forum_posts.asp?TID=11299). Covers pixel clusters, palettes, hue shifts, dithering, anti-aliasing, banding, pillow shading, and noise.
- [Aseprite Documentation](https://www.aseprite.org/docs/). Covers sprite structure, drawing, layers, animation, and export.
- [Aseprite Tilemaps](https://www.aseprite.org/docs/tilemap/). Explains reusable tiles, tilesets, and tilemap layers.
- [Aseprite Animation](https://www.aseprite.org/docs/animation/). Covers frames, timing, animation preview, and onion skinning.
- [Godot CanvasItem Documentation](https://docs.godotengine.org/en/stable/classes/class_canvasitem.html). Explains texture filtering choices, including nearest-neighbour filtering. Use the documentation for the actual engine when the project uses a different engine.
- [How to Write a Game Art Bible](https://blog.makko.ai/art-bible-game-development/). Discusses how a shared art bible keeps assets consistent across production.
- [Making a Game-Ready Sprite](https://www.wayline.io/learn/pixel-art/4). Discusses silhouette, actual-size readability, and consistent scale.

---

## Quick instruction to give the agent

Read this file and apply it to the current top-down game. First inspect the project and identify the main art defects. Then define the art direction and create a small reference set. Build a representative scene and test it in the game. Correct the art direction and assets based on this test. Only then expand the asset set. Make actual changes, verify the result, and report the changed files, tests, and remaining defects. Do not assume a fixed genre, camera, palette, resolution, tile size, or engine. Use the values that fit the project. Keep the art style coherent across every asset.
