# Investigation: LT Takeover's cost on the game's drawing thread (2026-10-18)

Reading only. No code was changed and nothing was built.

Question: what does LT Takeover do at the two load hitches LT Frame Lab
charged to it (about 99-121 ms on the title screen, about 144-159 ms on
entering the management screen), and what makes up its cost per frame in
a match (Frame Lab: 8.6-12 % of the drawing thread, about 1.3 ms of a
14-15 ms frame)?

Every claim carries one of the labels from `AGENTS.md`:

- **Seen in game (log)**: in a log written while the author played. The
  build and game version are given under "Sources".
- **Read from the code**: read in this checkout's source (the uncommitted
  1.3.6 working tree, which is the build that wrote the logs), in the
  SDK under `sdk/`, or in LT Frame Lab's source.
- **Guess**: plausible, not tested.

## Summary

1. **Title-screen hitch.** On the first frame of the title screen the mod
   reads the whole game executable from disk (about 87 MB) into memory and
   computes its SHA-256 fingerprint with Windows' bcrypt library. That
   fingerprint decides which game version's hooks to install. It runs once
   per game session. (Read from the code; the timing is seen in game in
   the logs.)
2. **Management-screen hitch.** When the save loads, the mod finds out
   which athletes are on the player's team. It does this by asking the
   game for the contract of **every athlete in the save**, one call per
   athlete, each returning a small JSON text that the mod then parses.
   This runs on the client thread in one frame. (The code path is read
   from the code; the hitch's timing and size are seen in game in the
   logs; that this loop is the cause is a strong guess.)
3. **Per frame in a match.** About half of what Frame Lab charges to LT
   Takeover is the game's own work. Frame Lab counts a sample as a mod's
   whenever that mod's code is anywhere on the call stack. Two of the
   mod's hooks wrap whole game functions: the match viewer update (about
   0.75 ms a frame) and the unit outline drawing (about 0.12 ms a frame).
   The game runs those functions either way. The mod's own callbacks cost
   about 0.68 ms a frame. Most of that is the HUD (about 0.28 ms) and the
   session buttons (about 0.19 ms) rebuilding and comparing their property
   text every frame, even when nothing changed. Steady-state writes to the
   game's UI are close to zero, apart from three unconditional ones listed
   below.

## Sources

| Source | What it is | Pairs with |
| --- | --- | --- |
| `%LOCALAPPDATA%\LTDirectControl\probe.log` | LT Takeover 1.3.6, game 0.6.4, 2026-10-18 from 05:24:58, PERF capture on | `frame_lab_2026-10-18_05-24-58.log` (same session: `NATIVE CONFIG os_thread=22740` = Frame Lab `client_thread_id=22740`) |
| `%LOCALAPPDATA%\LTDirectControl\probe_run1.log` | LT Takeover 1.3.6, game 0.6.4, 2026-10-17 from 19:05:25 | no Frame Lab log of that minute |
| `%LOCALAPPDATA%\LTFrameLab\frame_lab_2026-10-17_22-28-10.log` | Frame Lab 0.3.0 | no LT Takeover log of that session (`probe.log` has since been overwritten twice) |
| `%LOCALAPPDATA%\LTFrameLab\frame_lab_2026-10-18_05-24-58.log` | Frame Lab 0.4.1 | `probe.log` above |

The PERF capture of 2026-10-17 around 18:30-18:45 that the brief
mentions is not in any of these files. The oldest PERF lines found are
from 19:05 (`probe_run1.log`). The PERF capture was on during the paired
session, so its own small cost is inside Frame Lab's numbers there.

Frame Lab's address files list only offsets inside the game's executable,
not inside the mods. So no Frame Lab sample can be pinned to a function
of LT Takeover. Function names below come from the code and from the
timing in LT Takeover's own log. (Read from Frame Lab's address files.)

How Frame Lab assigns a sample to a mod: "the first mod module on the
stack" (`LTFrameLab/src/ring.rs`, `run_summary.rs`). A sample counts for
LT Takeover whenever any LT Takeover code is among the callers, even if
the code actually running is the game's or Windows'. (Read from the code.)

How LT Takeover's PERF line measures: `perf::frame` and `perf::time` in
`probe/src/perf.rs`. `pre_update`, `post_update` and `post_render` are
the wall time of the mod's three SDK callbacks, and they are the mod
total. `info`, `hud`, `tab`, `session`, `settings`, `shop`, `effect`,
`audit`, `hover` and `cursor` are named parts inside `post_update`.
Anything `post_update` does outside them has no name of its own.
`viewer_incl_native` is the mod's viewer hook *including* the game's
viewer update, and is labelled "not mod cost". The section timers always
run. The setting only adds the `PERF WORK` lines and turns on `PERF
SLOW` lines, which are written only on the battlefield. (Read from the
code.)

## 1. The title-screen hitch (about 99-121 ms, inside bcryptprimitives)

**What runs.** On the first client frame whose scene is the title screen,
`Client::post_update_inner` (`probe/src/client.rs`) calls
`Client::install_once`. That calls `native_adapter::install()`
(`probe/src/native_adapter/mod.rs`), which calls
`host::executable_sha()` (`probe/src/native_adapter/host.rs`). That
function:

1. calls `std::fs::read` on the game executable (87,228,416 bytes, per
   Frame Lab's `exe_bytes`) into one new 87 MB buffer;
2. hashes the buffer with `BCryptOpenAlgorithmProvider` and `BCryptHash`
   (SHA-256). Windows does that work in bcryptprimitives.dll;
3. compares the result with each supported version's `EXPECTED_SHA`, then
   runs that version's `windows::install`: PE identity check, layout
   guards, prologue byte checks, the patches, the minimap and shop hooks.

In the same frame, `install_once` also patches in the athlete stat rules
(`native_adapter::install_stat_rules`) when that setting is on.
(Read from the code.)

**How often.** Once per game session. `install_attempted` is set before
the call, so it never runs again. (Read from the code.)

**Timing in the logs.** The paired session's `probe.log`:

| Time (ms since epoch) | Line |
| --- | --- |
| 1792272298592 | `LANGUAGE game word for Close=...`: start of the first title frame's `post_update` |
| 1792272298684 | `MINIMAP installed`, `NATIVE PATCH_READBACK` (inside `install`) |
| 1792272298685 | `NATIVE install ok=true verified 0.6.4 SHA256` |
| 1792272298696 | `STATRULE installed sites=33` |

So that one frame spent about 104 ms in the mod: about 92 ms reading,
hashing and patching, then about 11 ms for the stat rules. Frame Lab's
title hitch in the same session is 121.4 ms with 89.7 % of samples under
LT Takeover (about 109 ms). In `probe_run1.log` the first PERF window
covers only the title screen (`scene=None`). Its `post_update` max is
88.2 ms, while every named part stays under 0.25 ms. That is the
fingerprint alone, in the part with no name. (Seen in game, in the logs.)

**Why bcryptprimitives and ntdll.** Frame Lab's leaf split for this hitch
is ntdll 55-59 % and bcryptprimitives 40-45 %. The bcryptprimitives part
is the SHA-256 itself (read from the code). The ntdll part is most likely
the file read and the 87 MB allocation: page faults and the kernel copy
are sampled at ntdll's system-call return (guess). Rust's `HashMap`
seeding also uses bcryptprimitives (`ProcessPrng`), but it is a few
microseconds per map and cannot account for 40 ms (guess, well grounded).

**Other start-up work, for completeness.** It is all read from the code,
and none of it is on the drawing thread for long:

- Sprite art is loaded on its own thread (`sprite_art::start`,
  `std::thread::Builder::spawn`). Its log line says `ms=162`, but that
  time is not on the client thread.
- The mod is loaded from the INIT line at .420 to the last start-up line
  at .471: 50 ms, before the title's first frame.

### Options (for the author to decide)

| Option | What it removes | What it risks |
| --- | --- | --- |
| A. Compute the fingerprint on a background thread started when the mod loads; `install_once` installs on the first title frame where the result is ready | The whole hitch, unless the title screen comes up before the hash is done (then a short wait at the title, on later frames) | Hashing touches no game address (`host.rs` says so), so the thread rules allow it. Installing must still happen at the title on the client thread. If the player loads a save before the hash finishes, the existing "Restart game to install adapter" path applies. That needs the "never install outside the title" rule kept exactly. |
| B. Memory-map the file instead of reading it into a new buffer | The 87 MB allocation and copy (the ntdll share, about half) | The hash still runs (about 40-50 ms, guess). Small code change in `host.rs`. |
| C. Remember the fingerprint per (path, size, modified time) in the mod's data folder and skip hashing when they match | Almost all of it after the first run | Weakens the "an unknown executable gets no hooks" guarantee: a changed file with the same size and time would be trusted. The other checks (PE timestamp and size, layout guards, prologue bytes) still run, but `AGENTS.md` asks to keep every fingerprint check. Not recommended. |

Recommendation: A, optionally with B. Both keep the full fingerprint.

## 2. The management-screen hitch (about 144-159 ms, allocator-heavy)

**What runs.** In every client frame where the game scene is `InGame`,
`post_update_inner` calls `Movement::load_own_team`
(`probe/src/movement.rs:142`) until the player's team is known. Once the
management data is available, that function:

1. calls `ctx.player_team_id()` and `ctx.team_name(team)`;
2. calls `ctx.athlete_ids()`, the id of **every athlete in the save**
   (all teams and free agents);
3. for each athlete calls
   `ctx.record_get_json(RecordKindV1::Athlete, id, "contract")`. The SDK
   reads the string in two calls (size, then copy), so each athlete costs
   a game-side JSON serialisation plus a new Rust `String`;
4. parses each JSON with `serde_json` (`own_selection::contract_team`)
   into a full `serde_json::Value` tree, then keeps the athletes whose
   `team_id` is the player's;
5. stores the team and logs `MANUAL own_team=...`.

It all runs in one client frame. (Read from the code and from
`sdk/mod-api-stable/src/client_ctx.rs`.)

**Timing in the logs.**

- Paired session: `CLIENT ... scene=Some(InGame)/Some(Main)` at
  1792272307350, then `MANUAL own_team=65 ... athletes={610..616}` at
  1792272307654. The scene line is logged at most once a second, so the
  two are within the same second. Frame Lab: an InGame/Main hitch of
  143.8 ms, 84.7 % under LT Takeover (about 122 ms). LT Takeover's own
  first PERF window, which covers the title and the management screen,
  has a `post_update` max of **132.6 ms**, while every named part's max
  is under 0.2 ms (`audit` 72 us, `hud` 154 us). The time was spent
  outside `apply_windows`, in the part of `post_update` with no name.
  `load_own_team` is in that part. The title frame (about 104 ms by the
  timestamps above) is smaller than 132.6 ms, so the max is most likely
  the management-screen frame. (Seen in game, in the logs; the reading of
  the max is a guess.)
- `probe_run1.log`: `MANUAL own_team` at 1792235183034, and that window's
  `post_update` max is **204.0 ms**, again with every named part under
  0.25 ms. (Seen in game, in the log.)
- Frame Lab 22:28 session: InGame/Main hitch of 159.3 ms, 77.5 % under
  LT Takeover. (Seen in game, in the log; there is no LT Takeover log for
  it.)

**Why "allocator-heavy".** The leaf split is ntdll 32-51 %, game 31-49 %
and VCRUNTIME140 12-17 %. That fits one game serialisation per athlete
(game code) and one allocation, copy and JSON tree per athlete (heap
calls in ntdll, copies in VCRUNTIME140). (Guess, consistent with the
code.) How many athletes the save holds was not counted. A save covers
whole leagues, so it is likely hundreds (guess).

**What else runs when the management screen opens, and why it is not the
cause.** All of these were checked in the code, and the logs rule them
out for that frame:

- `season_fix` and `result_audit`: inside the `audit` timer, max 72 us
  in that window (seen in game, in the log). Their big copy happens at
  match start (below).
- Shop catalogue, HUD, Tab, shop and stats windows: inactive off the
  battlefield; their timers stay tiny (seen in game, in the log).
- `acquisition::refresh` (`probe/src/acquisition.rs:95`): reads champion
  names and one translated label per champion when the session changes,
  or every 2 s while the level cap is unknown. It is small next to the
  athlete loop and has no timer of its own (read from the code; its size
  is a guess).
- Language: `lang::Probe::update` translates one word once a second
  (read from the code).
- Emote library, controls and settings files: read when the mod loads,
  not here (seen in game, in the log: the `EMOTE LIBRARY` and `CONTROLS`
  lines at start-up).

**It repeats after every match (read from the code; not in these logs).**
`Movement::reset_session` sets the whole state back to default, which
clears `own_team`. `Client::rearm_if_needed` calls `load_own_team` right
after the reset, so the full scan runs again on the first management or
pre-match frame after each match (`needs_rearm`: a released session plus
a pre-match scene, or a save exit). Neither log reached a second match
(no `MANUAL RESET` line), so this repeat has not been measured.

**The UI template spawns happen at match start, not on the management
screen.** The paired session's `PERF SLOW` lines (battlefield only) give
their size. Each lands on a different frame:

| Log line | Frame | Mod part |
| --- | --- | --- |
| 281 | 76.9 ms | `audit` 14.9 ms (the `SEASON series ... records copied` line is beside it) and `session` 1.3 ms (`SESSION portrait UI spawned`) |
| 323 | 33.5 ms | `hud` 17.0 ms (the HUD template is spawned) |
| 329 | 56.4 ms | `shop` 16.2 ms (the shop window is spawned) |

(Seen in game, in the log.) These are once per match and already spread
over separate frames.

**Two smaller hitches with no explanation yet.** On InGame/Lineup, Frame
Lab saw 65-67 ms with 24-36 % under LT Takeover. The paired PERF window
for Lineup to Match has a `post_update` max of 60.7 ms, again outside
every named part. Nothing in the code read here explains it. (Seen in
game, in the logs; the cause is open.)

### Options (for the author to decide)

| Option | What it removes | What it risks |
| --- | --- | --- |
| A. Read the roster from the team instead of from every athlete: `RecordKindV1::Team` exists in the SDK, and its record may list its athletes | The loop: one or a few calls instead of hundreds | Guess: whether the team record holds the roster, and under which field, is unknown. Check it first, for example with the TFM2 knowledge base or one logged `record_get_json(Team, id, "")`. |
| B. Spread the scan over frames (say 50 athletes per frame) and keep the partial result until it is complete | The single long frame; the total work stays the same | Ownership is known a few frames later. Callers already handle "not known yet" (`own_team` is `None`). The partial result must start over if the save or team changes mid-scan. |
| C. Keep the team across matches: after a match, re-check only the known athletes' contracts (about 7 calls) and do a full scan only if one has left | The repeat after every match | Missing a newly signed athlete until the next full scan. A full scan after a save exit keeps this safe. |
| D. Parse only `team_id` instead of a full JSON tree | Some of the allocations | Small gain. The game-side serialisation stays. |
| E. Move it to another thread | (not possible) | The SDK context belongs to the client callback (`docs/modding.md`, "Threads: the rules": the client thread does SDK calls and UI). Not allowed. |
| F. Do it later (at Lineup) | Moves the hitch; does not remove it | The pre-match chooser needs the roster. No gain. |

Recommendation: check A first (it would remove the loop). Otherwise
B with C.

## 3. The per-frame cost in a match

Steady InGame windows of the paired session (for example 05:26:30 to
05:27:30, frames 15-16 ms) give these figures. All of them are seen in
game, in the log:

| Part | Average per frame | Whose work |
| --- | --- | --- |
| `viewer_incl_native` | about 750 us | Mostly the game's own viewer update, run inside the mod's hook |
| `post_update` | about 600 us | The mod |
| of which `hud` | 270-300 us | The mod |
| of which `session` | about 190 us | The mod |
| of which `info` | about 23 us | The mod |
| of which `hover` and `cursor` | about 16 us each | The mod |
| of which `effect`, `settings`, `shop`, `audit` | 2-6 us each | The mod |
| of which unnamed | about 65 us | The mod |
| `outline_incl_native` | about 120 us (35,800 calls per 10 s, about 58 per frame, 2 us each) | Mostly the game's outline and draw-command code, run inside the mod's hook |
| `pre_update` | about 60 us | The mod |
| `post_render` | about 20 us | The mod |
| **Total** | **about 1.55 ms**, roughly 10 % of a 15.5 ms frame | |

The unnamed part covers movement input, `acquisition::refresh`,
`load_own_team` (a quick return once the team is known), `ui_theme`
refreshes, heartbeat, AI handback and `block_native_ui`.

The total fits Frame Lab's 8.6-12 %. LT Takeover's own "about 0.7 ms"
leaves out the viewer and outline parts because it labels them native.
(The addition is read from the logs; that the two measures match this
way is a guess, but a close one.)

**Why the viewer counts against the mod.** `view_hook`
(`probe/src/native_adapter/windows/view.rs`) runs on the client thread.
It times itself, does its pacing and camera work, then calls the game's
original viewer update (`original(...)`). Frame Lab's attribution is
stack-based (see "Sources"), so every sample inside the game's viewer
update is counted as LT Takeover's. The same holds for `outline_hook`
(`outline.rs`): it calls the game's outline function, then
`record_draw`. (Read from the code.) How much of the 750 us is the mod's
own work (`hook_entry`, `before_view`, `prepare_camera`, `frame_time`,
`after_view`, camera capture, `tooltips::resolve_pending`) is not
measured. These are small bookkeeping steps, so most of it is the game's
(guess).

### Work that runs every frame even when nothing changed

All read from the code:

1. **The HUD (`player_hud.rs`, `HudUi::apply`) rebuilds every property
   text every frame and compares it field by field.** Each node check
   runs `format!` for the node path and the property text,
   `ui_theme::themed` (a rewritten copy), `hud_motion::fields` (parsing
   the text), and one `format!("field:{path}:{key}")` string per field
   for the cache lookup (`hud_motion::unchanged`). The game is called
   only when a field changed, so steady-state writes are near zero. On
   top of that, `HudUi::props` stores
   `cache.insert(format!("properties:{node}"), source)` **every call,
   changed or not**: one more allocation and map insert per node per
   frame. The 23-node visibility loop alone (`combat` ... `shop_key`)
   does this every frame, then the item slots, skills, health ticks,
   texts and the four motion-pass writers (`hud_motion_pass::write`,
   which builds a `Vec<(String, String)>` each frame).
2. **`session_ui.rs:472` calls `ctx.ui_set_visible(PATH, true)` every
   frame without checking first.** The HUD checks `ui_visible` before
   writing; this one does not.
3. **`team_info.rs:116` calls `ctx.ui_set_properties(PANEL, "x: 0px; y:
   0px; anchor_x: 0.5; ...")` every frame while the takeover is engaged.**
   That is a direct, uncached property write on the game's own scoreboard
   panel. Lines 77-80 and 106-113 read the visibility of a handful of
   game nodes each frame and write only when it differs. That part is
   fine.
4. **`Client::block_native_ui` reads 9 game UI paths each frame**, plus
   every ancestor's visibility for each (about 25 path lookups) and up
   to 9 node rectangles.
5. **`ui_theme::refresh` calls `ui_exists` on 8 paths every frame**
   (`client.rs`, start of `post_update_inner`).
6. **`outline.rs` `record_draw` runs for every unit drawn (about 58 a
   frame), whether or not anything is hovered.** Each call builds a small
   `Vec`, takes the `SPRITE_DRAWS` lock and searches up to 512 entries
   one by one. Selection needs this data while you control an athlete
   (`native_adapter::sprite_draw`, 250 ms freshness).
7. **`session_ui` and the HUD read node rectangles for hover tests every
   frame** (`ui_node_rect` for the shop button, session buttons and
   portraits).

**What the game then does on the mod's behalf.** Every node the mod keeps
in the UI tree is laid out and drawn by the game each frame, outside the
mod's callbacks. Frame Lab counts that as `no_mod`, and nothing here
measures it. A rough count of node declarations in the templates (regex
over the template text; loops for item slots, skills and health ticks add
more) gives:

| Module | Node declarations (approximate) |
| --- | --- |
| HUD (`player_hud.rs`) | about 53 |
| `team_status.rs` | about 18 |
| `stats_ui.rs` | about 13 |
| `session_ui.rs` | about 12 |
| `emotes.rs` | about 8 |
| `screen_effect.rs` | about 1 |
| Shop | about 12 (hidden while closed) |
| Settings | about 40 (hidden while closed) |

That is on the order of 100-150 live nodes in a match (estimate).
Whether a property write with an unchanged value (items 2 and 3 above)
makes the game redo layout is unknown (guess: possible, especially for
the scoreboard's anchor and position). The mod's few immediate-mode
draws in `post_render` (ability previews, minimap frame, attack range,
click marks) are small at about 20 us (seen in game, in the log).

**Logging is not a factor.** `log_bytes` in a match is 1.5-4 KB per 10 s
and `log_io` totals under 0.3 ms per 10 s (seen in game, in the log).

### Ranked candidates for the 1.3 ms, by likely size

1. **The game's viewer update inside `view_hook`: about 750 us.** This is
   the game's own work, counted against the mod by Frame Lab's
   stack-based attribution. Nothing to remove; it is what the game draws
   anyway. (Size seen in game, in the log; that it is mostly game work is
   a guess.)
2. **HUD rebuild-and-compare (`hud`): 270-300 us.** It is all the mod's
   own string building and map lookups. Writes are near zero. (Seen in
   game, in the log, plus read from the code.)
3. **Session strip (`session`): about 190 us.** The same pattern, plus an
   unconditional `ui_set_visible` and hover rectangle reads. (Same.)
4. **The outline hook: about 120 us.** Mostly the game's outline work,
   plus `record_draw`'s allocation and lock for about 58 units a frame.
   (Same; the split is a guess.)
5. **The unnamed part of `post_update`: about 65 us**, including
   `block_native_ui` and the theme checks. (Same.)
6. **`pre_update` early input: about 60 us**, and **`post_render`: about
   20 us**. (Same.)
7. **Unmeasured: game layout and drawing of the mod's nodes, including
   the per-frame scoreboard property write.** It is outside Frame Lab's
   LT Takeover share, and its size is unknown (guess).

### Options (for the author to decide)

| Finding | Change | What it buys | What it risks |
| --- | --- | --- | --- |
| Viewer and outline attributed to the mod | None in the mod. Optionally have Frame Lab report a second figure: samples whose *innermost* mod frame is a hook wrapper around a game call, or "mod leaf only" | An honest number: about 0.7 ms of the 1.3-1.5 ms is the game's | None for LT Takeover. A Frame Lab change. |
| HUD and session rebuild every frame | Skip a window's whole apply when its inputs (snapshot, phase, cursor hover, status, motion still running) equal last frame's | Most of the 0.45 ms on quiet frames | Motion tweens and fades must keep running while animating. A missed input would leave stale UI on screen. Needs a careful input list per window. |
| The same | Cheaper cache: one stored string per node (compare the whole property text once), or numeric keys instead of `format!("field:...")`; drop the unconditional `properties:{node}` insert | Perhaps half of the HUD and session time (guess) | Field-level comparison exists so that a geometry change does not resend visibility (comment in `hud_motion.rs`). A whole-string cache must keep that. |
| `session_ui.rs:472` unconditional visible | Check `ui_visible` first, as the HUD does | One host write per frame, and any layout it causes | Very low. |
| `team_info.rs:116` scoreboard written every frame | Write on engage, then only when `ui_node_rect` shows the game moved it | One property write per frame on a game node, and any layout it causes | The game may reset that panel every frame. The per-frame write may be deliberate, to keep it centred. Check in game before changing. |
| `record_draw` on every unit draw | Record only while a selection consumer needs it (controlling, battlefield), and reuse buffers | Most of the mod's part of the outline hook | Selection reads data up to 250 ms old. It must stay fresh while controlling, or clicks pick the wrong unit. |
| `block_native_ui` and theme checks | Read the 9 game paths every few frames or on scene change; they rarely change | Part of the about 65 us | A panel that opens between reads would take battlefield clicks for one or two frames. |

## What remains a guess

- That the management-screen hitch is `load_own_team`'s athlete loop.
  The log shows the frame lies outside every named part, and the loop is
  the only bulk work there. A timer around `load_own_team`, or a count of
  athletes in the save, would settle it.
- That the ntdll share of the title hitch is the 87 MB read and
  allocation.
- How many athletes the save holds, and whether the team record lists
  its roster.
- That the full athlete scan repeats after every match (read from the
  code; not yet seen in a log).
- How much of `viewer_incl_native` and `outline_incl_native` is the mod's
  own code rather than the game's.
- Whether unchanged property and visibility writes make the game redo
  layout, and how much the game spends laying out and drawing the mod's
  nodes.
- What makes the 60-67 ms InGame/Lineup hitch.
