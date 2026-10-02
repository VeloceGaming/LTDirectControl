# Twentieth test: session buttons, attack range and a full battle

Fresh launch with Harbinger disabled. Confirmed 0.19 is preserved in
`dist/backups/0.19.0`. Select your lane before the match and check champion/side
at READY. There is no longer a 60-second running cutoff.

1. **Start:** click **Start control** below the top header. The battlefield
   should remain stopped until this click, then play with manual champion
   control. The button should become **Pause**. Button clicks must not move the
   champion or pan the camera.
2. **Pause/Resume:** click Pause while moving. The battlefield and clock should
   stop. Camera navigation, zoom and held Tab should remain available. While
   paused, try right-click, A, Q and B; these should not issue orders. Hold a
   gameplay input through Resume: it must require a new press to issue a new
   order. Pending movement/aiming is cleared by Pause. Resume continues the
   battle with manual control. Already committed native actions remain native.
3. **A range:** tap A and release. A circle should remain around your champion,
   following them and respecting camera pan/zoom. Left-click the battlefield
   to issue attack-move and hide it. Right-click, S, Esc, B or an ability command
   cancels it; camera movement and Tab preserve it. Champion-only mode must not
   prevent attack-move from attacking minions or change nearest-to-click choice.
   The circle shows the native effect's range; target body radii can affect the
   actual attack boundary. Report missing or clearly incorrect circles.
4. **Release run:** click **Return to AI** and check that the champion resumes
   AI behavior and native spectator UI/shortcuts return. This ends the controlled
   session. Ctrl+End remains an emergency release; Ctrl+Home starts/resumes as a
   backup. Re-taking control and controlling later sets in the same launch are
   not implemented yet.
5. **Full-battle run:** launch the game again, start control and play beyond
   60 seconds through the end of the first controlled battle/set. Do not use
   Return to AI for this run. If the champion dies, check that control recovers
   after respawn and other champions keep playing normally. Pause/Resume can be
   used during the run. Later sets in that launch retain native AI behavior.
6. **Result:** note the winner, your champion's final K/D/A and CS, and whether
   the result screen agrees. Keep the game open on the result/management screen
   for a few seconds so the read-only evidence capture can run. If practical,
   compare the saved match history or replay with the battle you played.

Please report any frozen transition, unexpected AI takeover, or mismatch at
the end, along with the visible result. The log and
`research/session-result-*-0.20.0.json` provide diagnostic evidence. Their
history/replay records are candidates for comparison, not proof that the
controlled simulation is what the game saved. If nothing is captured, that is
also useful evidence; the mod does not edit history or force a result.

Validation: 75 probe tests and 18 core tests pass; both packages pass all-target
Clippy with warnings denied. Tests cover scoped button requests, real worker
pause/wakeup, heartbeat recovery, no running cutoff, aiming cancellation and
held inputs across pause. Native drawing/click callbacks and full-battle results
still require this game test.
