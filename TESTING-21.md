# Twenty-first test: modded icons and death HUD

Fresh launch with Harbinger disabled. The prior 0.20 package is preserved in
`dist/backups/0.20.0`. Existing Start, Pause/Resume, Return to AI and combat/camera
controls remain. One controlled battle/set per launch still applies.

1. **While alive:** choose an enabled custom champion such as Candy Gel or
   Amazon. Check that all three skill icons appear, with W/R dim below levels
   3/5. After buying items through the game's existing AI system, check that the
   owned item icons appear, including expansion items. This update adds display
   support; it does not add manual purchasing. A base champion should retain
   its ordinary icons too; checking it can be a separate fresh launch.
2. **Death:** check that the portrait, skill art, inventory, level, gold and
   score remain visible. Skills should dim and the health strip should show
   **Respawn <seconds>** using the game's timer. Pause should freeze that timer;
   Resume should continue it. A rare absence of all AI callbacks retains the
   dead HUD but shows Dead instead of guessing the remaining time.
3. **Respawn:** normal health, cooldown display and manual control should return.
   Old movement/aiming/recall orders should not be replayed after death. Camera,
   Tab, A range, targeted skills and champion-only toggles should behave as before.
4. **Result evidence:** finishing a battle or returning to AI partway through
   should still reach the native result screen. Keep that screen open a few
   seconds; note your final K/D/A and CS. The capture now includes result-screen
   labels and a player sample restricted to the original simulation worker,
   frozen at battlefield exit. It remains diagnostic evidence, not proof of
   persistence or a substitute for checking saved history/replay.

Please report whether the icons appear while alive, whether they remain during
death, whether the countdown freezes during Pause, and whether control returns
on respawn. If an icon remains missing, name the champion or item (a screenshot
also helps). Logs record chosen asset paths, active item metadata and death/
respawn transitions; result evidence is in `research/session-result-*-0.21.0.json`.

Automated checks: 82 probe and 18 core tests pass, with all-target Clippy and
format checks. Enabled installed declarations and item-sheet tags are checked
without copying textures. Native icon drawing and death timing need this test.
