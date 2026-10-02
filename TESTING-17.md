# Seventeenth test: compact player HUD and hover cleanup

Fresh launch with Harbinger disabled. Select your lane before the match, check
the champion/side at READY, then Ctrl+Home. Ctrl+End releases. The **60-second
running guard** remains; the confirmed 0.16 package is backed up.

1. **Layout at READY:** bottom-center HUD should show your champion portrait,
   level, health, Q/W/R icons, gold, owned items, K/D/A and CS. It must leave the
   minimap and match score/clock usable. Spectator toolbars and portrait strip
   should be hidden. READY retains the detailed startup messages.
2. **Live information:** move, fight and cast. Health, gold and stats should
   update; cooldowns count down on the correct skill. Shift + Q/W/R highlights
   that slot while aiming. Recall shows a status message. During play, the
   large diagnostic messages should shrink to one top-left line. No items are
   purchased by clicking the HUD; it reports the game's existing inventory.
3. **Hover cleanup:** hover where the old hidden panels/portrait entries were.
   Their large tooltips should no longer appear. Hover your new skill/item
   slots: the HUD's own brief hints should appear and disappear normally.
4. **Click/camera protection:** right-click or confirm a cast over the player
   HUD: neither should issue a battlefield command. Drag/zoom must not begin
   there, while edge scrolling and minimap navigation still work. Hold Tab:
   the team panel stays centered and right-click-through remains available on
   battlefield areas outside the player HUD. Existing casting/recall controls
   should behave exactly as in 0.16.
5. **Release:** Ctrl+End or the guard should hide the player HUD and restore
   native spectator controls, including portrait hover interaction. If a death
   occurs, check whether the HUD shows the respawn state and returns correctly
   on revival. No deliberate death is required for this first layout test.

Please send a screenshot if an icon is wrong/missing, text overlaps, or the
HUD/minimap is misplaced. Also report your champion and whether you were at
READY, actively playing, dead or released when it happened.

HUD snapshots are taken every six simulation ticks, roughly ten updates per
second. Running samples older than 250 ms show unavailable data; READY may
retain the paused worker's initial sample. Borders describe cooldown/observed
effect state, not guaranteed cast eligibility. Some passive/locked slots have
no active effect; native validation remains authoritative. Unknown custom
champion/item icon metadata falls back to labels or a question mark. Native
icons are referenced from the installed bundle; no textures are shipped.

The first HUD uses English labels and brief control/category hints. Full skill
descriptions, item stats, localization and manual shopping remain later work.
Automated checks do not establish native UI parsing, visible layout or hover
timing; this game test does.
