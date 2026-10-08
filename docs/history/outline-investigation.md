# Hover outline investigation (after 0.51)

Date: 2026-10-08. Read-only: game bundle, preserved 0.6.3 executable, SDK. No
code was changed or built.

## What the engine can draw

**Renderer and shaders.** The renderer is Skia
(`engine-backend-skia\src\renderer.rs`). The shaders are built into the
executable, and the bundle contains none. A list in the renderer names:

- `asset/base/shader/default`, `flash`, `blur`, `greyscale`, `color_replace`,
  `color_shift`, `glow` and `post_process`;
- the parameters `flash` and `flash_color`, `sigma` (default 3.0),
  `intensity` (0.5), `ratio` (1.0), `hue` and `alpha`.

Mods cannot add their own shader without a native hook.

**The unit renderer already uses these shaders.** It is `0x230ec30`, in
`game-view/src/view/entity.rs`. It references `flash` at nine sites,
`color_shift` once and `glow` once (`0x2311395`, inside a 4-pass loop with a
float table). Gunner's view (`0x234b150`) and two other champion views also use
`flash`/`color_shift`. The trigger for the glow loop is not established. It
draws a sprite looked up by an animation name ending in `idle`/`loop`.

**Hit flash.** `flash` with `flash_color` is the per-unit hit-flash tint, which
matches `EntityView.flash_time`. The user rejected writing that state, as a
fragile workaround.

**SDK `draw_sprite`.** It has no tint or shader parameter, so it cannot make a
silhouette from the original sprite.

**UI nodes.** The native node kinds are label, empty, color, image, buttons,
canvas, slider and others. There is no animated-sprite node.

- Image nodes accept `shader: { name: ... }` (the game uses greyscale) and
  select a region only by `rect_tag`.
- Champion animation frames are `.fanim` frames, not rect tags.
- A UI image overlay therefore cannot show an arbitrary current frame without
  generated assets.

## Options

**A. Render-path redirect (recommended to investigate first).**

- At the point where `0x230ec30` chooses a unit's shader, substitute glow or a
  flash-style colour for the hovered entity ID only.
- This is a code redirect like the existing death-greyscale shader redirect
  (`SHADER_BYTES`). It writes no game state.
- The result is frame-exact, correctly layered and zoom-correct.
- Cost: one new fingerprinted render hook, which needs updating on game
  patches.
- Unknowns: the call shape and parameter plumbing at the chosen site; whether
  glow visually reads as an outline; per-frame cost.

**B. Overlay outline textures through `draw_sprite`.**

- Needs a verified view-entity reader. Its offsets come from the stop/move
  handlers: view `+138` map, entry `0x1d0`, animation String `+70/78/80`,
  time `+178`, speed `+17c`, position `+180..188`, facing `+1c6`.
- Needs per-frame outline textures generated from installed art, which
  conflicts with the no-copied-art rule unless generated locally.
- Unknowns: Game-map draw order and scale.
- This is more moving parts than A.

**C. Keep and refine the current ground rings.** No new risk.

## Next step, if approved

A read-only pass on `0x230ec30`:

- locate the normal sprite draw and its shader/parameter construction;
- find the identity of the entity being drawn at that point;
- identify the flash/glow parameter setters (`0x1401c91f0`, `0x1421725a0`).

That pass would show whether a single, guarded redirect can give one entity a
glow or outline colour. A prototype build would follow only with the user's
go-ahead.

## Read-only pass on option A (2026-10-08)

### The battlefield loop

`GameView::render` is `0x231f310`. It walks a list of `&EntityView` (`r13`)
and looks up a champion-specific view by name:

- **Custom view found:** it calls that view's vtable `+0x40` (`call rax` at
  `0x231fb1a`, for example Ninja `0x2349830`, Gunner `0x234b150` or Dual
  Blader `0x2347370`).
- **No custom view:** it calls the generic `0x230ec30` at `0x231faa6`.

Both paths return a `Vec<RenderCommand>` (0xd0 bytes per element) into
`rbp+0x3f0`, and they converge at `0x231fb1d`. The per-unit depth key is read
from `[r13+0x180]`.

The custom views call the generic renderer for the unit body. In total there
are ten direct CALL sites to `0x230ec30`:

- `0x231faa6`, `0x2347def`, `0x2348c33`, `0x234a2c9`, `0x234b334`,
  `0x2350312`, `0x2370fe0`, `0x1f077eb`, `0x215ae74`, `0x215c5c5`.

### Generic renderer

The generic renderer is `0x230ec30(out_vec, &EntityView, ...)`. It takes the
view as `rdx`, and no writes through it were found. Commands are built in this
order:

- `0x1401c9050`: create a sprite command (enum tag 3 or 4).
- `0x1401c76e0`: set the rectangle.
- `0x1401c91f0`: attach a shader by name (`default`, `flash`, `glow` and
  others).
- `0x1421725a0`: set a float parameter, e.g. `flash`, `sigma`, `intensity`.
- `0x142171ae0`: set a vector/colour parameter, e.g. `flash_color`.
- The command is then pushed with the grow function `0x14388dad0`, using the
  allocator at `0x142f99a60`.

The renderer read `[rsi+0x17c]` and `[rsi+0x180]` as floats. Two layouts would
fit, and they disagree about where the unit id sits:

- **Display data starts 8 bytes after the hash-map element start.** This
  matches the element layout seen in the stop/move handlers, where x/y sit at
  element `+0x184/+0x188`. The id would then be at `[view-8]`.
- **`rdx` is the element start itself.** The handlers' other fields (`+0x17c`
  speed, `+0x180` position) would also explain these reads. The id would then
  be at `[view+0]`.

This must be verified at runtime before use. A fallback is matching by
position.

### Proposed mechanism

Wrap the body renderer call sites. Only for the hovered entity id:

1. Call the original renderer a second time. This gives an independent,
   game-owned copy of the commands, with no manual cloning of the enum or its
   heap strings.
2. Re-style that copy's sprite commands with `glow` (sigma, intensity) through
   the game's own shader setter.
3. Put the copy before the normal commands, so the halo is drawn behind the
   unit.

This writes no unit data. A crisp alternative would be offset flash-silhouette
copies, which needs the command's position field (not yet located).

### Still unverified

- What `glow` looks like in the Skia backend: a coloured or a sprite-coloured
  halo.
- Whether the copy can be merged safely using the game's own Vec grow and
  allocator.
- The id offset.
- Whether the sibling calls in custom views draw extra parts (weapons or
  effects) that would also be glowed or missed.

### Cost

**Performance.**
- It does nothing unless a unit is hovered.
- When a unit is hovered, there is one extra renderer call for that unit, a
  few duplicated sprite commands, and a small blur.
- This is expected to be negligible, but should be measured, because of the
  user's earlier FPS sensitivity.

**Development.**
- One more read-only pass: glow semantics, Vec merge, id check.
- Then a prototype build, with one or two visual iterations.

**Maintenance.**
- About 13–15 new fingerprints: the ten call sites, the setters, the
  allocator/grow function and the layout guards.
- These need review on every game patch, using the existing migration tooling.

**Risk.**
- It runs in the per-frame render path.
- Guards keep the original path whenever anything is unexpected.
- A rollback build is kept.

## Final read-only pass (2026-10-08)

**Unit id.**
- `GameView::render` builds its draw list (via `0x141b4f5d0`) from the unit
  hash map at view `+0x138`. Each entry is `0x1d0` bytes: a u64 id key, then
  the display data.
- Other loops in the same function read `[bucket-0x1c8]` as display `+0`
  (team) and `[bucket-0x4c]` as x.
- So the renderer's `&EntityView` points 8 bytes past the key, and the id is
  at `[view-8]`. This is strong static evidence; the prototype should still
  log-check it.

**Coverage.** There are 15 direct CALL sites to the generic renderer
`0x230ec30`:

- `0x1f077eb`, `0x215ae74`, `0x215c5c5`, `0x231faa6`, `0x2347def`,
  `0x2348c33`, `0x234a2c9`, `0x234b334`, `0x2350312`, `0x2370fe0`,
  `0x2371024`, `0x2516bd7`, `0x2518cc9`, `0x2519be1`, `0x251b4b4`.

These cover `GameView::render` and every champion-specific view that appears in
a vtable: Knight, Lancer, Ninja, Gunner, Dual Blader, Circus Blade, Clown and
others. Each of those views draws the unit body through the generic renderer.
Extras that a custom view draws itself (weapons, effects) would not be
outlined.

**What glow looks like.** The game's own glow use is
`0x231124d`–`0x2311436`. It runs only for `tower`/`nexus` and stacks four
copies with (sigma, intensity) = (4, 1.5), (12, 1.2), (24, 0.8), (40, 0.4).
The glow implementation is in the Skia backend (`0x156750`, defaults sigma 3.0,
intensity 0.5) and blurs the sprite's own colours. Its look is the structure
crystal bloom. It is not a single chosen outline colour.

**Coloured outline.**
- The hit flash sets shader `flash`, the float `flash` and the colour
  `flash_color`, passed as a pointer to a float4 through `0x142171ae0`.
- Sprite command fields: tag variant 3 or 4; the source UV at `+0x68` (set by
  `0x1c76e0`); the draw position (x, y) at `+0x78`, copied by `0x1c9050` from
  the descriptor.
- So flash-tinted copies shifted by about one sprite pixel and drawn behind
  the unit give a crisp coloured outline. That is pixel-art style, close to
  League's look.

**Memory.**
- Allocation is `HeapAlloc(GetProcessHeap(), 0, size)` and freeing is
  `HeapFree(GetProcessHeap(), 0, ptr)`; see `16f3f7b`–`16f3f89`. That is the
  Rust System allocator, used for a command Vec with alignment of 16 or less.
- A merged list can be built as: copies, then originals, in a new buffer. Both
  old buffers are then freed after moving their elements.

## Prototype plan (approved before implementation)

Redirect the 15 CALL sites to one wrapper. That keeps the project's existing
CALL-redirect policy and adds no function-entry patch. Each site is
fingerprinted.

For each call, the wrapper calls the original once. Only when `[view-8]`
equals the hovered unit id, during active control on the live match, it also:

1. calls the original N more times to get independent copies;
2. restyles each copy's sprite commands with the game's own setters;
3. merges the result as copies first, then the originals;
4. sends anything unexpected (unknown tag, allocation failure, layout guard)
   back to the plain original result.

## Prototype implementation (0.55.1)

The user reported outlines only on towers, and some disagreement with the
ground construction marker. This first prototype is superseded by 0.55.2 below.

The 15 CALLs are fingerprinted against the installed 0.6.3 executable and
redirected to one wrapper. It calls the original renderer first. The wrapper
checks the selected unit's native id at `EntityView+0x100`, checks that the
view position is near the fresh picker position, and requires a Sprite-only
native command vector before asking the game for any extra render passes.
F10 switches between four shifted flash-coloured copies (crisp) and one glow
copy (soft). Ground rings remain. Ordinary units return the original vector
unchanged.

Each extra renderer invocation owns independent commands. The wrapper uses
the game's native shader/parameter setters, moves complete commands into a
new vector, then releases the vacated native vector buffers. It allocates the
destination before requesting extra passes so ordinary allocation failure
keeps the original drawing. A changed command shape disables the prototype
after moving its valid commands. Native rendering and appearance still need
an in-game test; automated checks verify only the static call sites and Rust
side of this boundary.

Two looks can be switched for the user's comparison:

- **outline:** four flash-coloured copies offset ±1 sprite pixel;
- **glow:** one or two glow copies.

The id is published from the existing hover picker through an atomic.

To verify in game: the flash colour semantics, the offset scale per zoom, the
draw order inside each unit's depth group, and per-frame cost.

## Updated cost

**Performance.** Nothing while nothing is hovered. While a unit is hovered,
there are 4 extra body renderer calls (outline) or 1–2 (glow) for that one
unit, plus their sprite draws. That is small, but should be checked with a
frame-time measurement.

**Maintenance.** About 20 new fingerprints: 15 call sites, 3–4 setter
functions and layout guards. They are reviewed per patch with the existing
migration tool.

**Development.** One prototype build carrying both looks, then one or two
visual tweaks. Removing the ground circles follows only after the user approves
the look.

## Mixed-command and single-hover fix (0.55.2)

Read-only inspection after the user test found 2,007 `unsupported` skips. The
generic renderer creates both Sprite (tag 3) and DrawLine (tag 8), while 0.55.1
required every command to be Sprite. The 0.6.3 DrawLine tag write at `0x2311692`
is now guarded alongside the existing renderer/setter instructions. Preserved
render type information shows DrawLine contains POD fields, with no owned heap
fields. The wrapper accepts these two variants, styles and moves only sprites
from extra passes, discards extra-pass POD lines, and moves the full original
command vector unchanged after the outline. Independent sprite ownership and
native vector-buffer release remain as before. Unknown original variants return
the normal drawing; an unexpected variant from an extra call disables the
prototype and forwards the valid, unstyled owned commands rather than guessing
their destructor. A malformed Vec/allocation failure also disables further
extra work or falls back according to the existing allocation guards.

The ordinary picker now sorts hostile before friendly, then champion before
other units within each side, then body-core/centre/ID. This includes enemy
minion over allied champion, as explicitly selected by the user. Champion-only
filtering and targeted skill eligibility still apply. Self is excluded from
passive hover. Targeted aiming can supply its one eligible unit or clear hover
on a miss; area skill previews retain their independent multi-unit geometry.

After current input and UI masks, post_update captures one marker tuple for the
frame. The cursor, native outline and post_render ground marker use its single
hover winner instead of picking again from different snapshots. Native identity,
position and team are published together through a short-lived mutex; no native
renderer/setter is called while that lock is held. The persistent orange attack
order ring remains distinct from hover and does not create another outline.

The supplied team policy is blue for allies (`#5CAEFF`) and red for enemies and
hostile neutral monsters (`#FF5B63`). F10 compares 1.5 and 2.5 sprite-coordinate
offsets using four coloured flash passes each. The uncoloured native glow mode
is removed because its available parameters do not provide the required team
colour control. Normal commands are appended after these copies. `OUTLINE`
samples now include `mixed` accepted draws to distinguish the fixed rejection
from other native issues.

Automated checks cover enemy/ally overlap order, champion-only filtering,
frame-stable shared feedback across intervening worker updates, UI/inactive/self
clearing, ally-skill eligibility, and mixed Sprite/DrawLine movement without
changing the original command bytes. The static hook checker verifies the new
DrawLine anchor. Native outline appearance, layers, offsets and performance
remain pending user gameplay testing; see `TESTING-55.2.md`.

## Tower-only correction (0.55.3)

The 0.55.2 user test still showed outlines only on towers. Its log contained
122 status samples, with final counters `drawn=590`, `unsupported=637`,
`near_id_misses=137`, `mixed=0`. The earlier DrawLine diagnosis was incomplete.

The generic renderer branches around `asset/base/sprite/circle` for tower/nexus
(`0x230f34b..0x230f391`). Ordinary unit circles use NinePatch, not Sprite or
DrawLine: constructor calls at `0x230f5e7`, `0x230f6b8`, `0x231038b` and
`0x231211a` reach `0x1c7880`, which writes tag 4 at `0x1c7a8a`. Optional labels
can also produce Text (tag 7). Rejecting either defeats the whole outline.

The wrapper accepts the 19 known RenderCommand variants, styles/moves only
sprites from independent extra passes, and appends the complete original
drawing unchanged. Every unused known command in an extra pass is dropped by
the game's own command cleanup at `0x1c5190`; its enclosing buffer is then
released separately. Original commands are moved once, never destroyed by this
cleanup. Text uses the String-capacity niche rather than a normal tag word.

The cleanup ABI (one borrowed command address, unit return) and variant dispatch
were checked against preserved named SDK drop glue and the live native shader
cleanup caller at `0x1ca533`. The routine releases owned String, shader, mesh,
text and vector fields without freeing its containing slot. The static verifier
checks all 669 bytes by hash, its 13-entry dispatch table, the caller, the
NinePatch tag write, existing setters and all 15 renderer redirects. Runtime
installation additionally guards the cleanup head/table/caller and tag write
under the existing whole-executable fingerprint. Evidence is saved in
`research/outline-command-cleanup-0.6.3.json`.

The merged buffer is allocated before extra renders, bounded by the existing
64-command per-pass limit. On a failed extra pass, valid extra commands are
cleaned up and the untouched original drawing is returned. Unknown tags are
never sent to a guessed destructor; an unexpected extra layout disables the
prototype. A malformed vector cannot safely be dereferenced/freed and may leak
that pass, while other valid passes are still cleaned up.

Regression tests cover Sprite/NinePatch/Text/DrawLine mixtures, sprite-only
moves, exactly-once auxiliary cleanup, fallback cleanup, unknown rejection and
the pass bound. Cleanup tests inject a spy rather than executing game code, so
they do not establish native rendering or native destructor behaviour in a
match. Diagnostics now name observed command variants and distinguish bad
vectors/no sprites from unsupported tags. Native testing is pending; see
`TESTING-55.3.md`.
