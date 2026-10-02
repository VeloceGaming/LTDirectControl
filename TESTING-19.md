# Nineteenth test: champion-only toggle and skill unlock shading

Fresh launch with Harbinger disabled. Select your lane, check the champion/side
at READY, then Ctrl+Home. Ctrl+End releases. The **60-second running guard**
remains; the confirmed 0.18 package is backed up in dist/backups/0.18.0.

1. **Both toggle buttons:** tap backtick, then Mouse 4 (the first thumb/side
   button). Either switches the same mode on/off; holding it should not repeat.
   A gold **CHAMPIONS ONLY** label appears while enabled. The mode starts off
   and resets when control is released. No configurable bindings are added.
2. **Direct right-click:** with the mode on, click an enemy champion amid
   minions. It should select the champion. Clicking only a minion/monster/tower
   should move to that ground point without attacking it. Toggle off and the
   same direct click should be able to attack non-champions again. A previously
   issued attack is not canceled solely by changing this targeting mode.
3. **Unit-targeted skills:** use a learned targeted skill, such as Lancer Q.
   With the mode on, a click/quickcast over only a minion should not cast;
   champions remain subject to the game's normal side/range/cooldown rules.
   In Shift normal-cast mode, toggling must keep the preview open. Invalid
   confirmation should keep aiming; toggle off and confirm on a valid minion.
   Ground/direction/self/no-target skills retain their existing behavior.
4. **Attack-move:** with champion-only enabled, A then left-click near minions
   should still attack minions, prioritizing the enemy nearest your click.
   Existing movement, recall, Tab and camera behavior should stay as before.
5. **Unlock display:** Q becomes learned at level 1, W at 3, R at 5. Below its
   level, the slot has a dimmed icon, gray border and Lv 3/5 label; its hover
   hint states the unlock level. On reaching the level, the slot returns to its
   ordinary cooldown/effect display. Reaching level 5 is optional in this short
   run; report the levels you actually observed.
6. **Release:** Ctrl+End hides the custom HUD, resets champion-only mode and
   restores the spectator panels/hover. The unwanted large popup should still
   stay hidden during control, as confirmed in 0.18.

Please report whether both physical buttons toggle correctly, whether you
tested amid minions, and any unexpected cast/attack target. Mouse 4 means
Windows XBUTTON1; mouse software may remap a thumb button, so report if it does
not register. The log records mode changes and the mode captured by each click
or cast. Focus return must not treat a held button as a fresh toggle.

Automated checks cover edge handling, overlap filtering, native rejection,
normal-cast confirmation, attack-move preservation and unlock thresholds.
Physical mouse/key detection and visible rendering still require this game test.
