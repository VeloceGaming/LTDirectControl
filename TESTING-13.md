# Thirteenth test: camera corrections and spectator hotkeys

Probe **0.13.0** corrects the follow-camera payload, captures middle-button
drags, moves edge-scrolling bands to the window edges, suppresses spectator
keyboard actions while controlling, and adds hold-Tab team information.
The full skill/gold HUD, attacks, attack-move, abilities and manual shop are
future work. Movement pacing and the 60-second running guard are unchanged.

Launch fresh with this probe enabled and Harbinger disabled. Select your lane
before the match with Ctrl+1..5. Wait for READY and check your champion and side.

1. **Team information:** hold Tab to show the team panel, release to hide it.
   Repeat while READY and after Ctrl+Home. It should not remain visible after
   releasing Tab or changing focus. Opening it should not cancel an existing
   movement destination. Right-clicking over the visible panel should not issue
   a world movement command.
2. **Space follow:** after Ctrl+Home, right-click a distant walkable destination
   and hold Space while your champion travels. The camera should follow
   continuously, not merely snap once. Release Space, then pan: the camera
   should remain free. Also check continuous Y lock on the correct champion.
3. **Manual navigation:** click the minimap or begin a middle-button drag while
   following. This should unlock. If Space remains held, deliberate manual
   navigation takes priority until you release and press Space again.
4. **Dragging over HUD:** show the team panel with Tab. Begin a middle-button
   drag on uncovered battlefield and continue across the panel. It should stay
   smooth. Starting a drag on the panel itself should not start camera panning.
5. **Edge scrolling:** with the camera free, move the cursor to the actual top
   or bottom edge of the game window, including over HUD controls. Up/down
   scrolling should work without finding a narrow exposed map border. Check
   left/right as well. Locked camera should remain locked at the edges.
6. **S and native shortcuts:** while moving, press S. The champion should stop
   without triggering the native spectator pause action. Native spectator
   seek/speed/follow shortcuts are suppressed while this session owns control.
   Mouse minimap navigation, UI buttons, wheel zoom and default +/- zoom keys
   continue to work. Custom remapped zoom keys are not guaranteed by this test.
7. **Release:** press Ctrl+End. The previous native team-panel visibility should
   return, native AI should resume, and native spectator keyboard shortcuts
   should work again. Quit after the test so the log can be inspected.

READY lasts at most 120 seconds and running lasts at most 60 seconds. Use a
second fresh launch if that is too short to check everything. No change is made
to saved keybindings; shortcut filtering is scoped to the bound battlefield
and ends on Ctrl+End, guard release or leaving the match.

Report continuous Space/Y following, Tab show/hide, dragging with information
visible, vertical edge scrolling, and whether S still causes a spectator pause.
The log includes input-hook traffic, suppressed spectator keys, Tab visibility
changes and native movement stops. Automated tests verify policy and argument
handling; the live test verifies the hook runs and the visible UI behavior.
