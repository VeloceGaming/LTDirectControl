# Investigation after 0.64.2: cursor, selection, emotes, performance

Date: 2026-10-08. Read-only review of source, the 0.64.2 log
(`%LOCALAPPDATA%\LTDirectControl\probe.log`, 2026-10-08 20:11) and the game
binary's imports/strings. Cross-checked against an independent review (Sol 6.1);
both agree on every point below. Nothing here is verified in game.

Side note (accepted as harmless, no change): the game's build plan for a
champion starts at four items and grows to six at that player's first purchase
(log: player 2 had 4 at tick 6, 6 at tick 400 right after buying a Dagger; AI
players already had 6 at their tick-3 purchase). Recommended therefore shows
four items until the first manual purchase.

## 1. Cursor flicker

The game is Rust/winit 0.30.5 and ships its own custom cursor
(`game_view::ui::cursor_ui::CursorUIRunner`). winit's `set_cursor` calls
`SetCursor` directly, outside `WM_SETCURSOR`. Our cursor
([cursor.rs](../probe/src/cursor.rs)) is reapplied only after `WM_SETCURSOR`
(a `WH_CALLWNDPROCRET` observer) and once per client frame, so a direct game
call shows the game cursor until our next frame or mouse move.

Second, separate source: we deliberately hand the cursor back over HUD,
minimap, outside the battlefield, and whenever a condition in
[lib.rs:592](../probe/src/lib.rs:592) is false for a frame.

Plan: trace ownership changes first (0.65), then make the game's own
cursor-setting path respect our active cursor (intercept the game's `SetCursor`
import) rather than reapplying more often.

## 2. Selection

Current areas are rectangles from idle/run frame sizes plus padding
([combat.rs:99](../probe/src/combat.rs:99)); atlas frames have no offset field,
so placement relative to the feet is approximate and corners are empty.

Plan for the selection pass (after 0.65):

- Body-shaped areas computed at startup from the installed art's opaque
  pixels (nothing copied), mirrored for both facings, correct sprite offset,
  modest forgiveness. The selection debug display shows them for tuning.
- No live-animation pixel picking (thin poses become hard to click; native
  view data unverified for 0.6.3).

Agreed hover priority (replaces the blanket champion-first rule; the user
pointed out League decides overlap dynamically, e.g. an enemy minion beats an
ally minion drawn in front of it):

0. Exclude: dead, untargetable (state still to be located), units the current
   skill cannot target, and non-champions while champion-only is held.
1. Side: enemies first; allies first while aiming an ally-target skill.
2. Own champion last, unless aiming a self-castable skill.
3. Structures last within a side.
4. Body hit beats forgiveness-margin hit.
5. Stickiness: the hovered unit keeps hover while the cursor stays on its body
   and nothing of a higher rank appears.
6. Depth: closest to the unit's own centre relative to its size (cursor on a
   small minion picks the minion; cursor on a champion's chest picks the
   champion; big monsters don't swallow champions in front).
7. Front-most, then stable ID, for exact ties only.

Cursor icon, hover outline and right-click target keep sharing the one winner.
Attack-move acquisition stays distance-based. Not recommended: current attack
target priority on hover; a monster-specific rule. Risk: dropping champion
priority changes fight feel; if champions become hard to click, add a small
champion depth bonus rather than restoring the hard rule.

## 3. League-style emotes

Feasible without new native hooks: SDK `draw_sprite`/`draw_svg`, native UI
templates with click events, `play_sound`, camera projection and configurable
hotkeys already exist. First version: hold a key for a wheel, select by
direction, release to show an image above the controlled champion (~2 s, pop
motion), optional sound, spam cooldown.

Restrictions: local only (no network sync, AI does not react, replays do not
keep them); no animated emotes (sprite sheets only have idle/run/attack/skill/
ult/hit/dead); art must be our own or user-imported, never Riot's; the wheel
must not issue move orders and needs pause/clipping handling; a free hotkey.

## 4. Performance

No frame-time measurement exists. Previous user RTSS reading: ~70–100 FPS with
small stutters; vanilla is CPU-heavy. UI writes are already deduplicated.

Proven waste (0.64.2 log):

- Movement steering hook ([native_adapter.rs:1872](../probe/src/native_adapter.rs:1872)):
  163,947,864 entries, 9,115 owned. Every entry takes the abilities mutex in
  `selected_key` before rejecting.
- Shop hooks ([shop.rs](../probe/src/shop.rs) `answer`): ~2.4 million mutex
  locks in the first ~20 s of play.
- The shop hook saw 12 distinct players, so concurrent (background)
  simulations on other threads also pass through these hooks; a shared lock
  across threads can make them wait. Plausible stutter source, unmeasured.

0.65 plan: cursor ownership trace; aggregated frame/callback/worker/lock
timing with sparse slow-frame lines; cheap atomic pre-lock rejection in the
steering and shop hooks, keeping every existing ownership check. Then a fair
comparison against a run with this mod disabled (RTSS frame-time graph).

Order: 0.65 diagnostics + pre-lock rejection, then cursor fix and measured
optimisation, then the selection pass, then emotes.
