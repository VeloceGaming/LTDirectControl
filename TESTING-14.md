# Fourteenth test: first combat controls and HUD cleanup

Probe **0.14.0** adds contextual right-click attacks, A-then-left-click
attack-move, a centered hold-Tab scoreboard with combat clicks through it,
automatic hiding of spectator detail panels, and mouse-wheel zoom.

Launch fresh with this probe enabled and Harbinger disabled. Choose your lane
before the match with Ctrl+1..5. Wait for READY and confirm the champion and side.
Ctrl+Home starts; Ctrl+End returns control to native AI.

The running test still ends after **60 seconds**. Use a second fresh launch to
test combat if the camera/HUD checks take too long. READY allows 120 seconds.

1. **Automatic HUD cleanup:** at READY, without clicking Hide Info UI, the
   side spectator champion cards, lower detailed statistics and spectator
   auto-camera/info controls should be hidden. Minimap, match header and
   playback/zoom controls remain for now. Hold Tab: the team scoreboard should
   appear in the center. Release Tab or switch focus: it should disappear.
2. **Click through Tab:** after Ctrl+Home, hold Tab and right-click an uncovered
   battlefield position beneath the center panel. Movement should work through
   the panel. The minimap and real HUD buttons still block world commands.
3. **Zoom:** with the cursor over battlefield, wheel up zooms in; wheel down
   zooms out. Test free camera and Y lock, then right-click a visible location
   at both zoom levels. The destination should match the cursor. Native +/-
   keys and zoom buttons remain available. Wheel over UI/Tab/minimap is left
   to native handling. The native zoom factor limits are 0.5–3.0 in steps of
   0.25; this build does not expand those limits.
4. **Right-click attacks:** approach a visible enemy minion, champion or tower
   and right-click its sprite. The champion should approach if necessary and
   repeatedly use native basic attacks. Ground right-click replaces that order.
   After the target dies or becomes invisible/untargetable, an explicit attack
   must stop rather than select another target. Report whether it attacks,
   chases indefinitely, walks too close between shots, or attacks the wrong unit.
5. **Attack-move priority:** with two enemies visible, press **A**, then left-click
   near the enemy farther from your champion. The yellow status line should show
   that attack-move is armed before the click. It should prefer the enemy nearest
   the clicked point. After a target dies, the order can acquire another enemy.
   Clicking an empty destination advances toward it and can attack nearby enemies
   along the way. Right-click, S, Esc or focus loss clears/cancels the order.
6. **Stop and release:** S should stop issuing movement/attacks without pausing
   the match. An already committed native attack animation can finish. Ctrl+End
   should restore spectator panels, scoreboard position/visibility, native AI
   and spectator shortcuts. Check Y, held Space, middle drag and minimap still work.

Attack acquisition currently uses a **120 world-unit radius** around either the
clicked point or your champion, then ranks those candidates by distance to the
clicked point. This is our initial mod policy, not a recovered native acquisition
radius. Only live, targetable enemies visible to your own team qualify. Native
validation decides whether to attack now; a rejected attack falls back to moving
toward the current target. Damage, cooldowns, range and attack animations remain
native. Ground movement and standing idle do not enable automatic attacks.

Sprite clicks use engine collision radius plus a small screen-space margin above
the ground point. This is an initial mod hitbox, not a copy of native sprite hit
testing. Report missed or ambiguous clicks so it can be adjusted from evidence.

The log records chosen order/target IDs, validated attack versus chase/hold,
wheel zoom changes, scoreboard layout failures and HUD takeover/restoration.
Automated tests cover targeting/order policy and existing camera/adapter guards;
they do not confirm the visible game behavior. Abilities, the full skill/gold HUD,
manual purchases and the eventual start/release interaction remain later work.
