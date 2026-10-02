# Agreed camera behavior

0.23 resets camera lock, drag, projection and held-key history per battle and
discards the finished viewer's native lease without dereferencing old addresses.
Each new session starts free; held Y/MMB/Space cannot replay an earlier gesture.
The user confirmed 0.22 death navigation. See TESTING-23.md for consecutive battles.

0.22 removes the living-position requirement from free-camera processing. The
selected identity remains available while dead; its position becomes optional.
Drag, edge pan, minimap and zoom continue, including during pause. Follow and
recenter requests require a living position. Y lock intent is retained on death
and resumes at respawn unless deliberate pan unlocked it. A dead-player drag
also interrupts held-Space intent, so respawn does not unexpectedly snap a free
camera. HUD drag exclusion and captured-drag continuation remain. No additional
native hook or field offset is introduced. See TESTING-22.md.

Camera is part of core direct control. This records the agreed design and the
0.14.0 combat/HUD extension, confirmed by the user. Ability previews in 0.15
reuse the same verified pan/zoom transform.
Source edits and builds still require the user's explicit go-ahead.

| Control | Behavior |
| --- | --- |
| Default | Free camera. |
| Free camera | Pan independently using window-edge scrolling or middle-button dragging. A drag started on battlefield continues over HUD until release. |
| Locked camera | Follow the selected champion on either team. |
| Y | Toggle free and locked camera. |
| Hold Space while free | Temporarily follow the selected champion. Release returns to free camera at the resulting camera position. |
| Hold Space while locked | Stay locked. |
| Minimap click or manual pan | Move the camera and switch to free mode if it was locked. |
| Start manual control | Center on the selected champion once, then use the chosen camera mode. |
| Hold Tab | Show the native team information panel centered; releasing or losing focus hides it. World command clicks pass through it. |
| Zoom | Mouse wheel over battlefield zooms within native 0.5–3.0 limits, 0.25 per notch. Native zoom buttons and +/- remain; targeting follows actual camera geometry. |

Movement and casting do not recenter a free camera. Camera controls remain
available while the battlefield is paused. HUD interaction must not issue
unintended movement or camera commands.

The user reports that the game already supports clicking the minimap to move
the camera. Reuse that behavior; verify how it interacts with follow mode rather
than replacing it based on an assumption. Existing native pan, follow, zoom and
shortcut behavior also need checking before choosing the integration.

Screen-to-battlefield targeting must use the actual current camera position,
zoom and battlefield viewport, including after native minimap navigation.

The user approved the mouse movement, explicit stop and camera build. Version
0.13.0 implements right-click ground movement, S stop and these camera controls.
Contextual attacks and attack-move nearest the click follow the live test.
I/J/K/L steering is retired.

The implementation reads current camera fields inside the existing native viewer
hook's borrow. Static disassembly of the installed 0.6.2 renderer confirms the
2048-square Game texture is centrally cropped into the battlefield viewport.
Picking therefore uses camera center plus the UI offset from viewport center
multiplied by camera extent / 2048, then converts to native position units.
Zoom, narrow/wide layouts and minimap exclusion use the current renderer's
geometry. Actual window/DPI behavior and native minimap interaction still need
the [thirteenth test](../TESTING-13.md).


Test 12 exposed a follow-payload error: native Follow stores lane at config+1c
and team side at +20. Current 1fcfd5d and native own-mid shortcut cb020e confirm
these fields. The prior SDK player-ID interpretation is superseded. The 0.13
adapter writes the selected side/lane pair, and camera policy keeps requesting
follow while Space is held. Only explicit minimap navigation or a captured drag
interrupts temporary follow. A generic native mode change no longer counts as
manual intent.

Edge bands are the outer 12 UI pixels of the 1920x1080 window surface, including
HUD and minimap overlap. This changes camera hover behavior, not HUD click
handling. Starting a drag on HUD remains blocked. Spectator keyboard handlers
are suppressed in the bound READY/running session; native mouse handlers remain.
Team-panel visibility is saved on takeover and restored on release. Skill/gold
HUD artwork and optional manual shopping remain later work.

## Approved 0.14 changes

During READY and control, the SDK hides wide_data/center_data spectator detail
panels and their info toggle, saving visibility for release. The existing
player_info panel is centered with SDK layout properties. It ignores UI events
and is excluded from combat-command masks while remaining a camera-drag/zoom
mask. Native layout position and visibility are restored on release. No saved
UI preference is changed. The full match header/bottom toolbar remained in 0.14.
Probe 0.20 keeps the header/minimap, hides the spectator toolbars and includes
the smaller 600 x 120 player HUD in camera and command masks. See player-hud.md
and TESTING-20.md. Session button bounds also block camera gestures and world
orders. During Pause, camera updates retain the original client delta while
playback receives zero delta, so camera navigation, wheel zoom and Tab remain
available. Resume does not change the chosen camera mode.

A Windows WH_GETMESSAGE hook on the owned client thread captures removed
WM_MOUSEWHEEL messages over the battlefield. It checks current coordinator
ownership, focus, fresh camera geometry and HUD exclusion. Those messages become
WM_NULL to avoid duplicate native zoom; all other messages pass on. The viewer
hook applies the accumulated notches to the verified native zoom field inside
its existing borrow. Hook installation/removal and visible zoom require live
test 14. Native keyboard zoom handler cb0acd/cb0afa/cb0b02/cb0b0e establishes
steps +/-0.25 and clamps 0.5–3.0.
