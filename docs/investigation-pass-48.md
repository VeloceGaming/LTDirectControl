# Investigation after 0.47.0: hover outline and animation cancelling

Date: 2026-10-08. Scope: read-only review of docs, mod source, SDK, runtime logs
and static disassembly of the preserved 0.6.2 and 0.6.3 executables. No source,
build, installed files or game memory changed. Nothing here was tested in game.

## 1. League-style hover accuracy and outline

League does not pick by exact pixels. Each champion has a forgiving selection
area, and a coloured outline shows which unit is under the cursor. Our picker
already follows that model: stable idle/walk body envelopes, head padding,
champion priority and a champion-only toggle. The user called 0.39 selection
"almost perfect" and accepted the later head and structure fixes. Per-pixel or
per-frame picking would make thin poses and swings harder to click, so it is
not recommended. Construction fit at several zoom levels is the only open fit
item.

The missing part is the outline itself. New concrete costs found in this pass:

- `StableSpriteParams` (SDK `draw_sprite`) has no colour or tint. The common
  trick of drawing the current frame several times, offset, in one flat colour
  is therefore unavailable. An outline needs separate outline-only textures for
  every frame of every champion sheet, including enabled Workshop packs.
- Shipping those textures would copy derived game artwork, which conflicts with
  the project's "no game artwork copied" rule. The compliant route is generating
  them on the user's machine from installed assets at startup or install time.
- Drawing the right outline needs each entity's current animation, frame time
  and facing. The client `EntityView` layout (`anim`, `anim_time`, `flip_x`,
  `x`, `y`, `id`) in `research/outline-view-layouts-40.json` comes from older
  debug data and is unverified for 0.6.3. The view entity list has not been
  located.
- The SDK documents a `Game` render map in match-world coordinates. Whether
  sprites drawn there sort with native unit sprites, and match their scale, is
  unknown.
- Option only: `EntityView.flash_time` (`Option<(f32, Color)>`) suggests a
  native whole-sprite tint used for hit flashes. Writing it for the hovered
  unit could give a frame-accurate tint highlight. It is a native view write at
  an unverified offset and could interfere with real hit flashes.

Assessment: worthwhile polish for crowded fights, but it is the most expensive
item on the list. If pursued, start with a bounded read-only spike: locate and
verify the view entity data for the controlled champion, and test one
`draw_sprite` on the `Game` map for draw order and zoom scaling. Stop and keep
the ground rings if either fails.

## 2. Animation cancelling

### Finding: the existing attack-to-skill release has never fired

0.35/0.36 added a release of the remaining basic-attack animation for a buffered
skill. It first needs an `ATTACK START` proof: `accepted_attack_delay` requires
action 3 with elapsed 0 **and** a new pending-effect queue entry from the same
call.

Static disassembly shows those two outcomes are on separate branches of the
native attack routine. The routine is 0.6.3 `0x16f3e30` and 0.6.2 `0x15b2700`:

- A dynamic check (`call [rdx+0xb8]`) chooses the branch.
- Queue branch: pushes the delayed effect (0.6.3 `+2a0/+2a8/+2b0`, element
  `0x38`, delay at `+0x10`). It then jumps past the action write
  (0.6.3 `16f410e -> 16f4158`; 0.6.2 `15b29de -> 15b2a28`). The action stays
  at 2 or less.
- Action branch: writes action 3, elapsed 0 and a target payload
  (0.6.3 `16f412c`; 0.6.2 `15b29fc`). Nothing is queued.

Runtime evidence matches. The logs contain `ATTACK OBSERVED ...
pending-effect-unconfirmed` every time and never `ATTACK START` or
`COMMITTED_INTERRUPT`: test-36 log (0.6.2) 132/0/0, current segments 390, 40 and
27 observed, 0 started. This is not a 0.6.3 regression. The release path has
been dead since 0.35. Smoother attack-to-skill weaving after 0.35/0.36 came from
command priority and buffering.

Not established:

- What the `+0xb8` check means.
- Which branch each champion's attacks take.
- Where action-3 attacks produce their hit. That is presumably the per-tick
  action handler at a scaled start timing, possibly from the Option at
  0.6.3 `+0x4a8/+0x4b8`. It has not been located for 0.6.3, so the post-hit
  commit point is unproven.
- Whether `+0x4b4` still identifies BaseAttack on 0.6.3.

Hypothesis only: the queue branch leaves action at 2 or less. The native Move
consumer accepts that, so those attacks may already not lock movement.

Do not loosen the start condition to accept either branch. On the action branch
the queue is empty, so release could happen before the hit. Attack cooldown
`+0xb0` is already spent at acceptance, so that attack would be lost.

### Value

- Post-hit move cancel (League's orbwalk): the native Move consumer rejects
  action > 2, so a move clicked during an action-3 attack waits for the whole
  animation. Releasing only after the hit, with cooldown kept, is fair kiting.
  The gain is small, a few ticks per attack at 60 ticks/s, and shrinks with
  attack speed. It is still noticeable for direct control. It depends on the
  commit point above.
- Pre-hit windup cancel: not recommended. Cooldown is consumed at acceptance on
  both branches, so cancelling before the hit only throws the attack away.
  Refunding it would mean removing queued native effects and restoring cooldown.
- Skill recovery cancel: defer. Dashes, channels and multi-stage effects make a
  generic release risky without per-family evidence.

### Recommended next step (requires the user's go-ahead)

1. Small diagnostic build: for each owned attack, log the champion, action
   before and after, elapsed, queue length before and after, and the BaseAttack
   field. One match with a melee and a ranged champion tells which branch each
   uses and whether queue-branch attacks already allow movement.
2. Read-only: locate the 0.6.3 action-3 tick that fires the attack effect, to
   get a real post-hit commit signal.
3. Then correct the gate per branch. Optionally let accepted move and stop
   orders, as well as buffered skills, use the same post-hit release. Keep
   cooldown and pending effects unchanged.

## Step 1 follow-up: how the display reacts to stop and move (read-only)

Static disassembly of the preserved 0.6.3 executable. Nothing was run.

The client's per-frame view update `0x921560` (anchored to
`game-view/src/view/game.rs` lines 909–997) loops over simulation events. Each
event is 0x38 bytes, and the loop dispatches through jump tables at `0x3a9e118`
(outer tags) and `0x3a9e1b0` (inner entity-event tags).

| Simulation event | Emitter | View handler | Display effect |
| --- | --- | --- | --- |
| Stop (outer tag `0x14`, actor id) | `0x13cccc0` (our `NOTIFY_STOP`) | `0x231e720` | Unless already idle: animation name → `idle`, anim speed 1.0 |
| Position (outer tag 17: id, x, y) | sim movement | `0x231e3d0` | Moved: animation → `run`, anim time 0, facing updated. Not moved: → `idle` |
| Attack (outer tag 2 → inner tag 3) | `0x13cdad0` | `0x231e8a0` | Starts the attack animation (body not reviewed) |

Neither handler checks whether an attack animation is playing. In the generic
view, the native stop event cuts an attack animation to idle, and the next
movement switches it to run. Our existing release path already emits that
native stop event. A post-hit release therefore should look like a real cancel
without writing any display memory. This still needs to be seen in game.

Gunner has dedicated view code (`game-view/src/view/entity/gunner.rs`;
functions `0x234b150`, `0x233f570`). This likely explains its walking-while-
shooting look. Ninja, Dual Blader, Knight, Lancer, Hammerer, Jiangshi, Monk,
Swordman, Clown, Circus Blade and Priest also have dedicated view functions.
Those bodies were not reviewed. Any of them may override the generic behaviour.

The same handlers expose the current client EntityView layout. This matters for
a future read-only outline reader:

- GameView `+0x138` is the hash-map control pointer, `+0x140` the bucket mask,
  `+0x150` the item count, and `+0x158` the hasher state. Entries are `0x1d0`
  bytes, with the entity id at entry `+0x0`.
- Entry fields: animation name String at `+0x70/+0x78/+0x80` (cap/ptr/len),
  anim time `+0x178` (f32), anim speed `+0x17c` (f32), positions
  `+0x180/+0x184/+0x188`, facing flag `+0x1c6`, and a skip flag `+0x1c4`.

These offsets are taken from the handler code. They would still need runtime
confirmation and migration-profile anchors before use. The mapping from anim
time to frame index has not been established.

Conclusion: no custom animation framework is needed for the generic case. The
remaining unknowns are on the simulation side: the post-hit commit point for
action-3 attacks, which champions take the queue branch, and the
champion-specific views.

## 0.50.0 diagnostic build

Static check of the 0.6.3 per-tick code: function `0x16f6cc0`, around
`0x16f7421`–`0x16f83da`. It computes
`min(start*100/max(speed+100,1), max(duration*100/max(speed+100,1),3) - 1)`
from the attack effect's start timing (`+4a8`, present unless `+4b8` is -1),
attack-speed bonus (`+3f4`) and a duration read through the `+568/+570` vtable.
It fires an effect, with attacker id and kind `+4b4`, when that value equals the
action payload word `+80`, not `+78`.

Not yet established:
- How that comparison connects to action 3's own tick.
- Whether `+80` is a counter or something else for this action.

The 0.50 build therefore records both payload words.

Added `probe/src/attack_trace.rs`. It writes no native memory and adds no hooks
and no behaviour changes. On every accepted owned attack (cooldown increased),
it logs `ATTACK TRACE_START` with:

- the branch (Queued / Locked / Unclassified);
- action before → after, both payload words, queue length and cooldown;
- raw kind, start timing and speed bonus;
- the queue path's delay formula (`expected_hit_tick`, a candidate only for
  locked attacks);
- any queued delay.

Existing observation points (attack, skill and move hooks) then log
`ATTACK TRACE_CHANGE` on action/queue changes, and `ATTACK TRACE_END` when the
attack settles, is replaced, or after 3 s. The trace is limited to 400 attacks
per match and 12 changes per attack. Samples only occur when an existing hook
runs, so millisecond values are upper bounds and tick gaps are possible.

Verification: 223 probe tests (6 new) and 18 core tests pass, along with fmt,
Clippy on both packages, the release build, 148 executable anchors, ABI 6 and
null-host rejection. UI/font assets are byte-identical to the verified 0.49.0
record. Pillow was unavailable, so the palette check was carried over by
SHA256 equality.

Installed with fingerprint checks; rollback is
`research/backups/0.49.0-before-0.50.0/installed`. Gameplay is unchanged by
design. It has not been verified in game.

## 0.50.0 results (user test, 2026-10-08, one launch: Ninja then Gunner)

Source: `C:/Users/j9010/AppData/Local/LTDirectControl/probe.log`. All attacks
were `source=input` and `kind=0`. No `COMMITTED_INTERRUPT` occurred, as
expected.

**Ninja (actor 20, 10 attacks): every attack took the Locked branch.**
- Declared start timing was 13 and the speed bonus 0. Payload `+78` counted
  0→19 and the lock cleared at about 20 ticks (302–357 ms at 60 ticks/s).
- Payload `+80` stayed at 100 throughout. It matches the attack-speed factor
  (100 + bonus) written by the attack routine, so it is not a counter. The
  earlier static reading that compared against `+80` should be treated as wrong
  or as belonging to another path.
- Cooldown was 50 ticks; 30 remained at release.
- Implied backswing: ticks 13–20, about 7 ticks or 117 ms per attack.

**Ninja move clicks during the swing were ignored until the lock ended.**
- In 9 of 10 attacks a Move click arrived mid-swing, at 99–342 ms. Many were
  after the declared hit tick (about 217 ms).
- The lock still ran its full ~20 ticks each time.
- This confirms that moving cannot cut the backswing today.

**Ninja attack then Q:**
- Attack 9 started at 0 ms.
- Q was pressed at +248 ms, after the declared hit.
- The attack lock ended at +325 ms.
- Q executed at +340 ms.

The skill therefore waited out the backswing, about 90 ms.

**Gunner (actor 20, 26 attacks): every attack took the Queued branch.**
- Action stayed 2 (moving), so there was no lock.
- Declared start timing was 16 and the speed bonus 10. Queued delay was 14,
  matching `16*100/110`.
- The effect left the queue after 214–274 ms, about 14 ticks.
- Move clicks as early as 14 ms after the start did not stop the shot leaving
  the queue.
- Gunner's walking-while-attacking is native simulation behaviour, not a mod
  issue.

**Still not observed:** the locked attack's actual damage moment. It is
inferred from the declared start timing (13). Releasing only when `+78` is
strictly greater than the scaled start keeps a one-tick margin.

## 0.51.0 implementation (post-hit release)

**Hit-tick proof.** `locked_attack_hit_tick` (native_adapter) replaces the
unsatisfiable `accepted_attack_delay` gate. It accepts an attack only when,
after the native consumer:

- action is 3 with elapsed `+78` = 0;
- no queue entry was added;
- the kind at `+4b4` is 0 (BaseAttack);
- the `+4b8` start timing is present;
- the speed factor `+80` is between 1 and 10000.

The hit tick is `start*100/factor`, which gives 13 for 0.50 Ninja. It is
recorded together with the starting cooldown and the start time.

**Release check.** `finish_attack_backswing` runs at the existing
move-consumer hook. It keeps the earlier conditions: the CC/forced-movement
gate, BaseAttack, an empty pending queue, and the attack's own cooldown. It
releases when `+78 > hit` and either:

- `buffered-skill`: a waiting learned/ready skill (the existing rules); or
- `manual-move-or-stop`: `MovementTest::manual_release_after(start)`. That is
  a ground Move click or S made after the attack start, with no later
  Attack/A-click order and no pending recall. Automatic chase/hold inputs
  never set it.

The release emits the native stop event and clears action 3 only. The static
view review says the generic view then switches to idle and, on movement, to
run. Ninja's custom view has not been reviewed.

**Tests.** 225 probe tests pass, plus 18 core tests. They cover the hit-tick
reader (including rejection cases and speed scaling), committed-attack timing
and identity, and manual-intent rules.

**Installed.** Fingerprint-checked; rollback is
`research/backups/0.50.0-before-0.51.0/installed`. Gameplay is not yet
verified in game.
