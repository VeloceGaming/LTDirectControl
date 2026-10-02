# Twelfth test: mouse movement, stop and camera

Probe **0.12.0** replaces diagnostic keyboard steering with right-click ground
movement. It also adds S to cancel movement and the agreed camera controls.
Startup coordination and the temporary 60-second running limit remain in place.

Launch the game fresh with this probe enabled and Harbinger disabled. Choose your
lane before entering the match with Ctrl+1 top, 2 jungle, 3 mid, 4 bottom or
5 support. Wait for READY and check the displayed champion and side.

1. While READY holds the battlefield, try Y to lock/unlock, hold and release
   Space, click the native minimap, and pan with a middle-button drag. These
   should move the camera without starting the match.
2. Press Ctrl+Home. The camera should center on your champion once. Right-click
   nearby walkable ground. A green destination marker should appear where you
   clicked, and the champion should move there after you release the button.
3. Click a farther destination, then press S while travelling. Check whether
   the champion stops promptly rather than continuing for another 1–2 seconds.
   Right-click a different direction and check that the new command replaces
   the previous destination. Do not use I/J/K/L; they no longer steer.
4. Pan or click the minimap, then repeat a ground click. Zoom using the native
   controls and repeat. The marker and destination should stay accurate. A
   camera change alone should not change the champion's destination.
5. Y should toggle persistent follow. Space should temporarily follow while
   free; releasing it should leave the camera where it is. Clicking the minimap
   or dragging manually should unlock. Edge scrolling should work while free.
6. Clicking HUD controls or the minimap should not issue a movement command.
   If practical, observe the champion near minions for unsolicited movement,
   attacks or ability casts, then press Ctrl+End to restore native AI and quit.

The test automatically releases control after 60 seconds. One controlled run
is supported per launch. You can check movement first and camera in a second
fresh launch if the time limit is too short.

Report where the champion actually went, whether S stopped it promptly, and any
camera action that broke targeting. If a click does nothing, mention its location
and the status text. The log records accepted/ignored clicks, camera changes and
actual native movement cancellation. The last click marker remains until S or
release, so you can check how it moves on screen when the camera moves.

This build implements ground movement and camera integration. Contextual attacks,
attack-move and ability controls still follow this foundation. S preserves an
attack/cast already underway and forced-movement effects. Native automatic combat
near minions and accurate targeting in your window configuration need this live
test; automated tests do not establish those visible results.
