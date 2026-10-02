# 0.26.1: portable installation

On the C:-only laptop, close the game, extract the release's
`lt_direct_control_probe` folder into `Teamfight Manager2/mods/`, enable the mod,
and restart. Keep Harbinger disabled. No D: drive, development project,
activation files, or Rust installation is needed.

1. After drafting, the battlefield should wait with five own-team portraits.
   Select a portrait and click the triangle. Check right-click movement,
   attack-move, abilities and camera controls.
2. Check the 0.26 graphical HUD and tooltips as described in
   [TESTING-26.md](TESTING-26.md). This patch retains that build's gameplay/UI.
3. Press Win+R and enter `%LOCALAPPDATA%\LTDirectControl`. The folder should
   contain `probe.log` with an `INIT` line identifying `probe=0.26.1`.
   A subsequent launch retains the previous recording as `probe.previous.log`.
   If local app-data storage cannot be opened, logs use `%TEMP%\LTDirectControl`.
4. Returning to AI and completing the match should still work. Read-only
   result diagnostics are stored in the selected log folder's `research`
   subfolder; game save/result handling is unchanged.

The automated checks cover C:-based path selection, missing/invalid app-data
paths, an inaccessible primary folder, log rotation, and unavailable logging.
They do not establish gameplay or rendering on the other laptop.
