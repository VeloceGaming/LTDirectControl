# Emotes and performance — 0.80.1

## Emotes

Hold **T**, move the pointer in a direction, and release. Centre is GG;
up Nice, right Hype, down Oops, left Focus. Escape or a new right-click
cancels. Settings > Keybinds can change both bindings. Settings > Emotes
can disable the wheel or enable its quiet confirmation sound (off by default).

These are five original vector faces. The catalogue in `probe/src/emotes.rs`
is independent of selection policy, animation and native UI construction.
`tools/generate_emotes.cjs` regenerates their PNGs, sound and fingerprints.
Settings > Emotes > Library assigns built-in or imported images to any of
the five slots. Four user-provided emotes are also bundled: Bomber Nice,
Clown Son, Ghost Wajaja and Monk No Kills. These are separate 256 × 256 PNG
assets and appear on the first launch after installation, without importing
or copying them to AppData. The original five wheel assignments remain the
defaults. No Riot emote artwork is bundled. Custom sounds and animated
imports are deferred; the optional shared confirmation sound is unchanged.

### Import your own images

1. Choose **Settings > Emotes > Library > Open folder**. Normally this is
   `%LOCALAPPDATA%\LTDirectControl\emotes`; it follows the same storage
   fallback as the mod's log/settings folder when Local AppData is unavailable.
2. Copy static PNG files into that folder, then press **Refresh**.
3. Select an image, select a wheel position, then **Assign**. **Apply & close**
   saves the draft; Cancel discards assignments, and Restore this page resets
   assignments and display options. Importing files is independent of Apply.
4. **Restart the game** to load newly added or changed textures. Pending
   entries say Restart. They can be assigned before restart; their wheel slots
   use the corresponding built-in image until the imported texture is ready.

| Rule | Limit |
| --- | --- |
| Format | Static PNG; square and transparency recommended, not mandatory |
| Dimensions | 1–256 pixels on each side; 256 × 256 recommended |
| File size | At most 1 MiB per PNG |
| Library | Five original defaults, four bundled images, plus up to 64 imports; top-level files only |
| Names | Unicode supported; at most 68 characters including `.png`, no control characters |
| Animation / custom sound | Deferred; GIF, APNG, sprite sheets and sound sidecars are not played |

The normalizer trims fully transparent margins, fits the longest visible
side to 224 pixels and centres it on a transparent 256 × 256 canvas. It
preserves aspect and filters using premultiplied alpha to avoid dark edge
fringes. This equalizes visible bounds, though thin or sparse artwork can
still look visually smaller than a solid face. The texture resolution does
not change the configured on-screen size. An opaque background counts
as artwork. Originals are never resized or overwritten.

Imports are scanned at startup and on Refresh, with at most 1024 directory
entries inspected. Invalid files display an issue and log the full rejection
reason. File links and reparse points are rejected. The mod never executes
imports. It stores normalized, content-versioned PNGs and a small catalogue
in its installed `ui/imported` folder. If the mod folder cannot be written,
built-ins still work and the library reports the error. Updating the mod
with the supplied installer preserves this cache.

The SDK has no runtime texture upload/reload API. This version uses the
game's ordinary packaged asset paths and deliberately requires restart;
Refresh does not imply that the native texture has loaded. The catalogue
marks previously staged files eligible on the next boot. Native rendering
must be checked in-game. Removing an original hides it from the library
and activates the slot's built-in fallback; its assignment is retained so
putting the same filename back restores it. Bundled originals in the import
folder reuse their ready bundled entry; personal variants with the same
filename replace that entry without creating duplicates. Removing a variant
restores the bundled image. Renaming a file creates a new library identity.
Unreferenced cache files are cleaned without deleting
textures that this running process might still use.

One emote is displayed above the living controlled champion for 120 played
ticks (2 seconds), with a default 90-tick cooldown (1.5 seconds). A new emote
replaces the current image. The shared cooldown is adjustable from 0.25 to
6 seconds. The wheel does not
pause or replace the champion's existing order. The first 0.24 seconds pop
and settle; the final 0.3 seconds fade. All durations follow match playback,
so pause freezes them and a hitch advances directly to the new age, with no
animation catch-up loop. There is no network/replay record or AI reaction.

Settings > Emotes > Display controls height (0–240 UI pixels, default 108) and
scale (50–200%, default 100%; an 88-pixel settled image). Height is the gap
from the champion's ground position to the image's lower edge before the
small animated upward lift. It is not an exact health-bar attachment.
Fixed screen size is the default. Follow camera multiplies image size, gap
and lift by the native camera zoom (0.5–3.0), with 100% as the reference.
Changing image scale alone does not change its lower-edge gap.

The silent live preview uses the same geometry and pop/fade animation, with
a champion marker and sample camera zoom buttons. The sample uses one fixed
display scale so 200% and 300% samples differ. At extreme settings it pans
to keep the image visible and labels when the champion is below the preview.
Its clock runs while Settings pauses the
match; it never changes the match camera or actual emote state. Apply saves
the draft, Cancel discards it and Restore this page resets emote options.
The shared packaged sound lasts 160 ms, less than the 250 ms minimum cooldown.
The SDK offers no per-sound stop operation; custom sounds are not supported.

The wheel is blocked by settings, shop and skill/attack-move aiming. It
cancels on focus loss, death, loss of control and match/identity changes.
Cooldown survives death, focus loss and AI handover within the same identity;
a new session clears it. Held keys cannot reopen it on focus return or
takeover. Cancel clicks, Escape and skills held while choosing are consumed
through release, preserving the previous world order. A transparent native
input plate prevents choosing clicks reaching buttons beneath the wheel.
The normal spectator T shortcut is not claimed in AI control.

The overlay is preloaded once control is available, reused, and changes only
necessary native properties. An idle, already-hidden overlay issues no UI
setters. UI spawn failure retries at most once every 2 seconds and affects
only emotes. Missing art/sound must not change combat. The floating image is
hidden outside the visible battlefield/minimap boundaries, and the wheel
fits the 1920x1080 virtual UI surface even when opened near an edge.

## Performance measurements

Settings > Advanced > Debug > **Capture performance measurements** enables
detailed aggregate timings. Normal log detail is enough. Turn it off for
ordinary play: measuring hot paths itself has overhead. Records are emitted
every 10 seconds in the user's normal `LTDirectControl/probe.log` folder.
No additional diagnostic writer thread or unbounded event queue is created.

`PERF WORK` records include counts, average/max microseconds, p95/p99
histogram upper bounds, and totals for:

- SDK AI callbacks on all worker threads, including time waiting for the
  session gate, plus selected unit scanning and combat decisions.
- Owned ability observation and steering work; shop decisions including
  their lock. Existing counters still report fast rejections and lock waits.
- Attack, skill, move and outline hooks **including original game work**.
- Log file writes and rotation, separate from the logger lock wait.
- Input capture to published-frame playback. This is not measured visible
  movement, attack/projectile onset or monitor latency. The trace queue is
  bounded to 64; stalls can omit samples. Capture can continue after the
  usual first-300 diagnostic limit.

These scopes overlap and must not be added together. `mod_total_ms` still
means the three client callbacks only, not whole-mod CPU time. Frame timing
is the interval between client updates, not GPU presentation. Work histogram
bins are powers of two in microseconds; the last bucket is greater than
8,388,608 us. Samples straddling a report boundary can land in adjacent
windows; use several windows, not one, for a comparison.

To produce a numeric report without copying player/team names:

```powershell
python tools/summarize_performance.py path/to/probe.log --output report.json
```

The report preserves individual windows and build versions. It does not
average window percentiles into a fictitious whole-session percentile or
interpret unavailable measurements as zero cost.

## Changes made without slowing control updates

- One selected-player entity scan supplies both skill and attack targeting.
  Existing live/targetable/visibility tests are retained; attack targeting
  filters out allies and the actor. Entity names are read once per scanned
  unit. No SDK entity or borrowed pointer survives the callback.
- Enemy, hover and ability snapshot vectors retain capacity between updates.
  Attack targeting consumes an iterator, avoiding a temporary enemy vector.
- Input and combat policy use an immutable shared snapshot of applied
  settings, rebuilt on Apply, rather than repeatedly copying JSON. Large
  acquisition documents remain outside this input snapshot.
- Disabled attack diagnostics do not build trace strings or take the attack
  tracer mutex. Selected frequent verbose messages are constructed lazily;
  severity checks do not allocate lowercase copies of each message.
- The left-match rejection precedes the session read lock, preserving the
  intended 0.78.2 background-match fast exit.
- Existing client frame counters use a stack array instead of allocating a
  vector each frame. Detailed simulation/hook timers are opt-in.

Input polling, combat updates, native safety checks, one-frame worker lead,
heartbeat recovery, shop decisions and authoritative game attack timing
are not throttled. Synchronous file I/O is now measured; a background log
writer is deliberately deferred until captures show it is needed, since
that adds shutdown, queue-overflow and lost-diagnostic risks.

## Baseline and validation limits

The starting source was 0.78.2. Its baseline release test run had 363 passing
tests and two file-save failures from restricted temporary-directory access.
Using normal file access resolves that environment issue. Older PERF logs
primarily measured client callbacks and cannot establish whole-mod CPU cost
or a new-build speedup. No fresh native-match baseline or lower-spec hardware
capture was performed automatically. Record both comparison runs with the
same detailed-capture setting to account for profiler overhead.

Automated checks cover input ownership, lifecycle, cooldown/paused clocks,
bounded geometry, image validation/normalization, restart/cache lifecycle,
snapshot filtering, assignment/settings persistence and report
parsing. They cannot verify native rendering, sound loading or gameplay.

The user should test these equivalent scenarios with the same save, game
version, resolution and champion/item mod set:

| Scenario | Check |
| --- | --- |
| Vanilla items and champions | Wheel, sound, existing orders, 4-slot shop and casting |
| Riot items and a Workshop champion such as Harpy | Same checks; expanded inventory, dynamic ranges/tooltips |
| Crowded fights, many projectiles, late match | Several timing windows; p95/p99/max update intervals, CPU/GPU usage and memory trend |
| Shop/settings/Tab and key capture | Wheel ownership, pause ownership, no clicks or held skills leaking |
| Death/respawn, Alt-Tab, AI return/reclaim | Clear visuals, no stale wheel or accidental cast; spectator T still works |
| Leave mid-match, next match, save reload | No lingering overlay/input mask; no old identity/position/cooldown |
| 720p, 1080p, 1440p and different window scaling | Wheel edges, pointer direction, HUD/minimap overlap |
| Profiling off/on; no-emote and frequent-emote runs | Separate instrumentation cost from emote cost; one image/sound at a time |

Compare vanilla/no-direct-control, installed-but-AI control and direct control
runs. Differences can include native game workloads, so captures indicate
where to investigate rather than proving causation by themselves. Average
FPS alone misses stalls and delayed controls. Monitor memory over multiple
matches, not just one fight.

Restricting CPU resources or injecting stalls can expose problems on a fast
PC, but an FPS cap alone does not emulate weak hardware. No power plan,
affinity, driver or operating-system setting is changed by the mod or report
tool. A real older/laptop system is still needed before making hardware
support or performance-improvement claims.
