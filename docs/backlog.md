# Backlog

What is left to do, as of 1.2.3 (2026-10-15; 1.0.0 was the first public release). Finished and declined items
are listed at the end for reference.

Sorting rule for feature ideas (user, 2026-10-15): would a standard MOBA give
it to every player? If yes, it can be a main feature. If it is an advantage a
MOBA does not give (information the game hides), it belongs in a clearly
labelled cheat/fun section and is never promoted as the mod's identity.
Ideas should serve the match itself (in-game experience), not additions
around it; the other mod's ping feature is the model of a well-scoped
addition (not to be copied).

## Open

1. **Match freeze (0.77.2 and 1.0.0, once each; both with the Riot item
   mod).** The simulation worker stops mid-match while the screen stays
   responsive. The 1.0.0 report read `last_step=Outside`: not the mod's
   per-player work or pacing wait. 1.0.1 logs the worker's call stack
   (`WORKER STALL stack n/3`), marks the native hooks as steps and counts
   the players begun in the frozen tick; if it recurs, read those lines.
2. **Skill previews.** 0.75.2 decodes Ice Mage R (cone), Bard R (aura) and
   Exorcist R (area at the cast), matched to `champion_info`; Gunner Q/R/W,
   Bard Q, Exorcist W and Executioner W are single-target or self buffs and
   correctly show no area. 0.75.3: Dancer R as a fan of 4-10 blades from
   her kill stack; Cavalry Knight R (a speed buff and road) shows no
   direction guide. Bard W (changed by the 0.74.1 landing rule) is untested in game.
   The user keeps observing every champion's preview over time.
   Known limits, accepted for now: buff-dependent skills (SwitchByBuff)
   show the unbuffed version; dashes that stop at the first unit (Rush) have
   no known end; timed dashes are drawn one body wide.
   Keep the PREVIEW TREE / PREVIEW ROSTER logging until previews are done.
   **Rethink (2026-10-16, user):** the live reader cannot cover Workshop
   champions by design (170 custom DLL effects across 30 items; about 25
   JSON shapes it does not decode), and the user will not describe champions
   to agents. Findings and a proposed design, not yet decided:
   [investigation-preview-sources.md](investigation-preview-sources.md)
   (read champion files first, then watch and learn; unit-targeted skills
   keep a reticle; no "could not read" crosshair).
   **Decided 2026-10-20 (author):** watching play was measured and cannot
   give widths or areas (investigation section 6); read the champion files
   first. Stage 1 (1.5.0-dev2): the file decoder reads the remaining JSON
   shape types. Stage 2 (1.5.0-dev4): read the 8 data base champions from
   the bundle at start instead of the baked copy. Stage 3 (1.5.0-dev5, the
   author's option A): a skill not understood shows only its cast style,
   an arrow toward the aim or a landing ring, instead of a crosshair.
   Also 1.5.0-dev3: dashes follow the skill's main purpose.
   **Later, with its tradeoff:** replace the live reader for the 60
   compiled base champions (about 37 version-specific entries per game
   version, 7 anchors and about 30 effect addresses on 0.6.4) with a table
   generated once from the live reader (shape kinds and sizes) plus the
   bundle's `champion_info` numbers read at start. Gains: no preview entries
   to carry per game version. Loses, until the active in-game balance patch
   can be read: patched numbers (the patch settings allow range ±10 %,
   projectile speed ±10 % and, for a few skills, radius, width or
   projectile radius to change), and a game update that changes a size is
   missed until the table is regenerated. Best done after reading the
   active patch is solved.
3. **Ability tooltip numbers ("…").** Native descriptions are enabled for
   all champions since 0.69.1, with the old resolver as fallback. Of 204
   localized entries, 6 are confirmed in logs and 198 are unobserved. Needs
   broader in-game testing. See
   [investigation-tooltips-native.md](investigation-tooltips-native.md) and
   `tools/audit_tooltips.py`.
4. **Emotes and performance validation.** The user accepted the built-in
   wheel and display controls (0.79.0–0.79.1). Static PNG import/library
   assignment is implemented in 0.80.0; native imported texture loading
   after restart still needs user testing. Custom sounds and animated
   imports are deferred. Detailed performance capture/reporting is available;
   lower-spec hardware and extended compatibility runs remain outstanding.
   See [emotes-and-performance.md](emotes-and-performance.md).
5. **Stats panel values from items.** Haste, lifesteal and penetration are
   summed from buffs; check them against the game's own panel with items
   that grant them (`STATS own raw` lines, every 10 s).
6. **UI refactor (later, when the user is motivated).** Option 1 of the
   customisation discussion: let modders restyle the HUD without editing
   Rust templates. The background colour (0.76-0.77.2) is the built-in
   limited customisation meanwhile.
7. **Translations.** All 17 game languages ship (1.0.0); the wording is
   untested by native speakers except Traditional Chinese, and Hawaiian is
   best-effort. Fix reports go into `probe/lang/<code>.json`.
8. **The game's own frame cost (investigation only, later).** The game
   runs at a modest frame rate without the mod, also on a PC that runs
   large 3D games well (user observation, 2026-10-11; not measured). Measure
   where a frame goes with the mod's features off: the viewer and worker
   hooks can time the game's own steps, and background matches compete for
   CPU. Decide afterwards whether anything is worth attempting; a cost in
   how the game draws is not something a mod can realistically change.
   Since 1.0.6 a low frame rate no longer slows the match.
   **2026-10-17, seen in game (the author):** 240 Hz monitor, vertical
   sync off, frame limit 144 (read from `config/game/base.json`), yet 58
   to 80 fps depending on where the camera looks; one core near 100 %,
   the rest idle; fullscreen a few frames better than borderless. Read
   from the code: the game draws through SDL 2.26 on its main thread. So
   the limit is the game's own per-frame drawing work on one thread, not
   a cap. **Decision (the author): measuring and any improvement are a
   separate mod**, not part of LT Takeover; this item is closed here.
   Constraint for that mod: two mods cannot hook the same instruction, so
   it must not take LT Takeover's viewer or worker hook sites.
   **Answered 2026-10-17** by a profiler capture (LT Frame Lab,
   `docs/investigation-frame-profile-2026-10-17.md` there): the game draws
   with Skia over OpenGL on one thread; the time is spread over hundreds
   of functions (hottest 0.7 %), the driver (30 %) and the allocator
   (13 %). Structural, not patchable by a mod. LT Takeover's own cost in a
   match is 0.7 ms per frame (its capture, same day).
   **Seen in game 2026-10-17 (Frame Lab 0.3 sampler, all mods on):** LT
   Takeover is the only mod with a measurable share of the drawing thread
   in a match, 9–12 % counting the game and Windows work done on its
   behalf (about 1.4 ms of a 14 ms frame; 2–3 % in its own code). Every
   other mod's DLL is under 1 %. Two load hitches were charged to LT
   Takeover: 159 ms entering the management screen (78 % of its samples
   in LT Takeover's call chain, allocator-heavy) and 99 ms on the title
   screen (100 %, inside `bcryptprimitives`). Candidate item: find what LT
   Takeover does at those two moments and move it off the drawing thread.
   Done in 1.3.7 (background fingerprint, chunked roster scan; seen in
   game 2026-10-18: title share 100 % → 33 %, management 159 ms at 78 %
   → 55–66 ms at 0–30 %).
   **Seen in game 2026-10-18 (Frame Lab 0.6, all threads, all mods):**
   across the simulation threads LT Takeover's call chains account for
   0.74 of a core in a match (by-mod rule, so the game's work inside its
   hooks is included), more than Smarter Combat AI (0.66) or Drakes
   (0.41); the game itself 2.8 cores of 4.76 sampled. The hot path is
   known from the PERF lines: the steering hook and its owner fast-reject
   run about 170,000 times a second on the simulation threads. Candidate
   item: measure the hooks' own share on those threads (Frame Lab's
   address file for `other` threads maps to the hook relays) and cut the
   per-call cost or the call count. Context: with NVIDIA's threaded driver
   mode, load on other threads costs drawing-thread frames (A/B runs the
   same day), so this is frame rate, not only CPU.
   **Closed 2026-10-18 (seen in game, Frame Lab 0.6.2 own-code figure):**
   of the 0.77 cores by call chain, 0.07 is the mod's own code; the rest
   is the game's steering and path-finding running inside the hooks,
   which the game runs with the mod off too. Nothing to cut on the
   simulation side. The drawing-thread side was compared with the POV
   mod (0.8 % own code with more UI, against our 2.4 %): the gap is the
   UI-tree design (about 100 nodes rebuilt as property text every frame).
   1.3.11 made the unchanged check cheap (seen in game: HUD 379 → about
   200 µs, strip 199 → about 120 µs per frame); skipping the whole
   rebuild was declined as unsafe, and moving the HUD to direct draws
   is a rewrite not worth a 2 % frame gain. The optimisation pass ends
   here by the author's decision.
9. **Enemy threat info on hover** (idea, 2026-10-12). Hovering an enemy
   shows which of its skills are on cooldown. Its attack range ring was
   part of the idea, but the other direct-control mod already has that, so
   the cooldowns are the part that would be new. Cheat/fun section by the
   2026-10-15 rule.
10. **Mechanical polish** (ideas, 2026-10-12). Input buffering (the next
    skill is queued during an animation and fires on the first possible
    tick); cast at maximum range when the cursor is beyond a skill's range,
    as a setting; spectate a teammate while dead, with a key to cycle. All
    three lean on the mod's low input delay. 2026-10-15: buffering and
    spectate-while-dead are main features (standard MOBA behaviour); see
    items 16 and 17.
11. **Practice tool** (idea, 2026-10-12). A sandbox toggle: reset
    cooldowns, set gold and level, refill health, spawn a target dummy.
    The no-cooldown test cheat (Home+End) is the start of it; it would also
    shorten skill-preview testing after a game patch. Cheat/fun section.
    Stage 1 in 1.2.8 (development, 2026-10-16): Advanced > Debug >
    "Practice tool". Enemies hold still, health and cooldowns refill,
    Home+Insert lines the enemies up at the cursor, `practice_lineup` in
    controls.json drafts named champions, and a readout of the last shots
    (needs the stat rules installed). Declined by the author (2026-10-16):
    controlling an opponent to test ten champions a match (much of the mod
    assumes the controlled athlete is on the player's team). Open: a lab
    room without a save; the game has a `GameTest` scene (`SceneKindV1`
    5) that the official interface cannot open, so whether a mod can reach
    it needs native research first. 1.2.10: everyone but the player holds
    still and pulled enemies are pinned. 1.2.12 (2026-10-16, the author):
    out of the settings window, a debug tool like no-cooldown; switched on
    only by `"practice_mode": 1` in controls.json (read at game start), so
    the lineup still applies from the draft. Kept rough on purpose.
12. **Controller support** (idea, 2026-10-12; low priority). The user
    barely uses a controller and would have to tune an input type they do
    not play with.
13. **Ultra rapid fire mode** (idea the user liked, 2026-10-12). A match
    option where everyone has almost no cooldowns; the no-cooldown test
    cheat is most of it. Cheat/fun section.
14. **Career wall** (idea the user liked, 2026-10-12). A page of the
    player's manual-control records: kills, pentakills, most-played
    champion, win streaks. Fits the direction the user named the same day:
    playing as one athlete, like a sports game's career mode. Lower
    priority since 2026-10-15: the match itself comes first.
15. **Athlete stat rules in manual matches** (the user's first priority;
    design agreed 2026-10-15, not built). Findings in
    [investigation-stat-rules-0.6.4.md](investigation-stat-rules-0.6.4.md);
    the withdrawn 1.2.1 attempt is on the local branch
    `backup/1.2.1-stat-rules`. Decisions:
    - **Collision is fixed.** No dynamic collision of any kind in a
      manually played match: the stat margin is one constant for every shot
      the player fires, at every kind of target, and the player's own
      Dodging term is zero. Starting constant 5 game units, the game's own
      clamp floor; the user wants to see it in game before judging it (draw
      the registering width in the skill preview; a testing overlay of
      collision bodies in the cheat section). AI-versus-AI shots stay
      vanilla.
    - **Enemy reaction to the player's shots** follows the gap
      caster Accuracy − dodger Dodging, mapped into a human band. Starting
      curve: gap −100 → 6..9 ticks, 0 → 10..14, +100 → 16..22 (anything
      near 400 ms was rejected as a guaranteed hit). Floor and ceiling are
      Advanced settings while testing; defaults frozen afterwards. The
      game's deterministic per-projectile choice is kept.
    - **The player's Dodging** must still matter. The game already uses it
      in the AI aim roll (investigation, section 4); whether to replace that
      roll with the same gap is undecided. A telegraph of incoming enemy
      shots whose lead time scales with Dodging is wanted as an option,
      behind a toggle, since it changes how the game plays; its value is
      limited to slow projectiles (section 5). A mix of both is expected.
    - **Build approach:** intercept the derived values at the producers,
      not the 21 collision consumers (collision prototype finding); scope
      every change to the controlled athlete, check identity before any
      lock, keep an off switch; A/B test with Crossbowman Q as the
      prototype document describes.
    - Open before building: whether the learned dodge policy reads the
      stats directly. (Which collision shape each skill type takes is no
      longer needed, 2026-10-16: 1.2.11 hooks the 11 shape-A floors as
      well, so the margin setting reaches every shot; see the 0.6.4
      investigation, section 7a.)
16. **Casting bar** (accepted 2026-10-15, with item 10's buffering).
    A bar under the champion showing the current action's progress with
    the hit tick marked and a pip for the buffered skill; display only.
    Guess: skill actions carry the same duration and hit-tick fields the
    attack hook already reads, to confirm on 0.6.4. What the user finds
    lacking in today's one-slot, one-second buffer is still to be named.
17. **Death recap** (2026-10-15). While dead, the damage sources of the
    last few seconds. A standard MOBA feature, so a main feature; pairs
    with spectating a teammate while dead (item 10).

## Done

- Ground ring rollback (1.3.19, 2026-10-18; read from the code, rollback not yet seen in game): restores the original 1.3.16 debug-based ring drawing and click pulse, removes the dedicated redesign module and its tests. Character-shaped outlines, compatibility settings and unrelated HUD changes are preserved. Play checklist: `local/testing/TESTING-1.3.19.md`.

- **Rejected / superseded, 2026-10-18:** both Ground ring redesigns below are removed in 1.3.19 at the author's request. **Seen in game (1.3.18, author; game version not stated):** champion rings too wide and fading behind champions unsuccessful. The old implementation records below are retained as history; their earlier "not seen in game" statements describe their build stages.

- Ground ring readability and finish (1.3.18, 2026-10-18; read from the code, not seen in game): League-inspired tapered front crescent, readable rear arc, pale inner edge, larger attack clamps and cut-corner tower-base brackets. Supersedes the thin four-arc appearance after the author's 1.3.17 feedback; compatibility and picking are unchanged. Play checklist: `local/testing/TESTING-1.3.18.md`.

- Ground ring redesign (1.3.17, 2026-10-18; read from the code, not seen in game): dedicated segmented team-coloured foot halos, a stronger attack marker with side chevrons, fixed-size brightness feedback on clicks and tower-base brackets. Selection/debug geometry and compatibility policy stay separate. Play checklist: `local/testing/TESTING-1.3.17.md`.

- Larger charge marks and one-time ready flash (1.3.10, 2026-10-18; read from the code, not seen in game): centred 10 × 8 px marks and a 400 ms pale-gold inset slot glow. Multi-use skills flash from zero to available uses, not for extra charges while usable. Red refusal feedback takes priority. Play checklist: `local/testing/TESTING-1.3.10.md`.

- Cooldown edge endpoint (1.3.9, 2026-10-18; read from the code, fix not seen in game): the moving line and its trail stop at the ready cue's 2 px inset, preventing the coded backward step when recovery finishes. The author's 1.3.8 report and remaining in-game check are in CHANGELOG.md and `local/testing/TESTING-1.3.9.md`.

- HUD slot feedback and motion pruning (1.3.8, 2026-10-18; read from the code, not seen in game): dark red unavailable-press feedback confined to the bottom skill slots, multi-use charge marks, match-start effects removed, and the shop's opening sweep removed while its fade stays. Author's play checklist: `local/testing/TESTING-1.3.8.md`.

- Public release 1.0.0: mod ID `lt_direct_control`, name "LT Takeover:
  Open-Source Direct Control", author Layton; the game's SDK no longer in
  the repository (`tools/prepare_sdk.py`); Steam Workshop description by the
  user. The Home+End no-cooldown test option stays in as a small easter egg
  (single player only).
- Languages (0.82.0-0.83.2): every mod text in all 17 game languages, Auto
  detection from the game, all-script font fallbacks, duplicate click
  callbacks after window rebuilds dropped (`ui_click`).
- Whole build plan from the start (0.78.0-0.78.1): the game's buy-new
  decision runs once (answer discarded) so Riot fills slots 5-6; the shop
  opens by itself afterwards.
- AI control (0.78.2): the game's layout toggle works; speed or View Match
  Result hands the match to the game; left matches skip the mod's per-tick
  work.

- Stats panels (0.77.0-0.77.3, accepted): own stats left of the Q slot
  (C toggles), League-style left-click selection with a target frame
  (face or tower / jungle / minion glyph, health and shield, 12 stats).
- Background colour (0.76.4-0.77.2): one setting moves every surface,
  frame and hover grey by the same offset (`ui_theme`); text inks fixed.
  Stat icons (game sheet plus the mod's sprite sheet) also in shop filters.
- Skill preview redesign (0.75.0-0.75.3, accepted) from the user's design
  (kept outside git); the 0.75 Drawing test setting was
  removed after acceptance (0.77.3 housekeeping).

- Selection pass (0.72.0-0.72.3, accepted): bodies measured from the art,
  placed with the game's draw data; the agreed League-style ranking.
- Skill previews (0.71-0.74.3): 13 native effect types decoded; landing
  and post-dash placement rules.
- Testing aid (0.74.1-0.74.3): Home+End, no cooldowns (+900 haste).
- F11 also pauses (0.67.0).
- Shop "Vanilla order" option, default on (0.67.0).

## Declined

- The design's 550 ms dash reveal (user, 2026-10-09: "means little and can
  backfire").

- Esc on the shop also opens the game's Esc menu (user, 2026-10-09: too
  much work for a small thing). The SDK cannot consume keys and the game
  opens the menu through a path not yet found.
