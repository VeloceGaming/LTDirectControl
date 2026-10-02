# Sixteenth test: normal casting and B recall

Probe **0.16.0** retains the previous combat, camera and HUD controls. Fresh
launch, Harbinger disabled, choose your lane before the match, check champion
and side at READY, then Ctrl+Home. Ctrl+End releases control. The running guard
still ends the test after **60 seconds**; use a fresh launch for another run.
The confirmed 0.15 package is preserved in dist/backups/0.15.0.

1. **Normal cast:** tap Shift + Q, then release both keys. The range/aim display
   stays visible. Move the cursor and left-click a valid target to cast once;
   acceptance dismisses the display. Lancer Q needs an enemy unit. Empty-ground
   confirmation should leave aiming open, allowing a later valid click. It must
   not cast by itself if a unit enters the cursor or cooldown ends.
2. **Switch/cancel:** Shift + another skill switches aiming. Right-click,
   quickcast Q/W/R, A, B, S and Esc cancel it and perform their normal action.
   Holding an ability or left-click must not repeat casts. Real HUD/minimap
   clicks do not confirm; the centered Tab scoreboard remains click-through.
3. **Camera:** pan/zoom or hold Space while aiming. The display stays active,
   and a later confirmation uses the cursor's position with the current camera.
4. **Recall completion:** walk away from the fountain, press B and leave the
   champion alone. The previous move/attack order stops and the native recall
   should complete. Hold B during a separate recall: it must not restart the
   channel every frame. Watch for the "Recalling" diagnostic status.
5. **Recall cancellation:** on separate B presses, try right-click movement,
   S and Esc. Each should cancel recall. A valid skill should follow the game's
   native interruption rules. An invalid skill must not leave manual control or
   secretly fire later. An unavailable B request should report rejection and
   require another press. Focus loss cancels aiming/pending requests but does
   not deliberately interrupt an already running native recall.

Please report the champion, key/action and visible outcome for any failure.
For recall, distinguish "never started", "started then cancelled" and "never
finished". The log records Return submission, observed native action changes
and explicit cancellation events.

Indicators show nominal cast range and aim, not the full damage area. Cooldowns,
level unlocks, animation locks, native target rules and recast behavior remain
authoritative. Requests expire after 250 ms and are not queued until legal.
Automated checks cover input routing and native memory/event boundaries;
visible recall completion and persistent aiming still require this game test.
