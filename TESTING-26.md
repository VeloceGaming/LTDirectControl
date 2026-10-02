# 0.26: portrait selection and graphical HUD

Restart the game with Harbinger disabled. The verified 0.25 package is preserved
in dist/backups/0.25.0; its last recording is archived under research.

1. After drafting, the held battlefield should show five own-team portraits
   centered on screen, with no role/champion names. Click a different portrait:
   its border turns yellow and the bottom HUD changes to that champion. The
   clock stays held. Click the triangle to start. Ctrl+1–5 also works while held;
   role changes are locked after starting. Choices remain own-team on either side.
2. Move and cast with the chosen champion. The old champion must retain AI, and
   no old movement, recall or skill command should carry over from selection.
3. Check grayscale panel backgrounds and frames, numeric K/D/A with deaths red,
   coin/CS graphics, dimmed locked abilities and all vanilla/modded item art.
   Y changes the camera icon; M4/backtick turns the targeting icon yellow;
   B changes the recall indicator. These HUD indicators use their shown keys.
4. Hover a skill or occupied item slot briefly. A description should appear in
   the game's language when the installed pack provides that translation, and
   disappear when leaving. Empty slots do not show a tooltip. Formula parameters
   not exposed by the SDK appear as ellipses; they are not estimated. Check text
   wrapping and report any untranslated reference/path or clipped description.
   Tooltip bodies ignore mouse events and do not add a movement-blocking area.
5. Click the pause bars, then the triangle to resume. Click the chip to return
   to AI. Ctrl+Home still starts/resumes; Ctrl+End releases. The preparation
   screen no longer automatically starts AI after 120 seconds. Missing client
   heartbeat and failed startup still release safely.
6. If you die, the portrait remains visible but dimmed, with a numeric respawn
   countdown. Team death timers and death camera panning should still work.
   In a later set, the selector must show that set's actual five champions.

Please report selection/controls first, then any visual or tooltip issue.
Automated checks do not establish native in-game UI rendering or button hit tests.
