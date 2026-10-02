# Twenty-fourth test: target commitment, markers and team deaths

Restart the game with Harbinger disabled. The working 0.23 package is preserved
in `dist/backups/0.23.0`. Start control normally.

1. **Direct A-click:** near enemies/minions, press A and left-click directly on
   an enemy champion. If it moves outside attack range, your champion should
   chase that same champion instead of attacking a nearby minion. A-click on
   empty ground still selects an enemy nearest the clicked spot. If a committed
   target dies or leaves vision, that attack order stops without switching targets.
2. **Markers:** hover an enemy: a pale gold ring marks the picked unit. After
   ordering an attack, an outer orange ring stays on the target when you move
   the cursor away. With champion-only enabled, ordinary hover/right-click
   skips minions; while A is armed, minions remain eligible. Hovering the HUD or
   minimap should not pick battlefield units. Check panning/zoom briefly.
3. **Deaths:** five portraits per team should appear below the top scoreboard,
   in lane order. Your selected champion has a gold border. Dead portraits dim
   and show seconds until respawn; the portrait remains visible. Check an ally
   and enemy death, and pause during a countdown: it must freeze, then resume.
   Continue to another set if convenient: portraits must reflect the new draft
   and actual blue/red sides. They disappear on Return to AI.
4. **Exorcist, if drafted:** Q/Purification needs an allied champion currently
   affected by crowd control. A rejected Q now states that requirement. Aim at
   such an ally while Q is ready. If that still fails, report the target and its
   status; cooldown, range and eligibility still come from the game.

Please report whether direct A-click chases correctly, whether the two markers
identify the expected target, and whether all ten portraits/countdowns appear.
There is no need to repeat every previously confirmed control.

Validation: 96 probe tests and 18 core tests pass; both packages pass all-target
Clippy with warnings denied and format checks. Tests cover direct-target chase
and loss without fallback, unrestricted A acquisition, hover masks/freshness,
death/pause/native countdowns, identity retention, roster resets and native
ally-CC validation. Rendering and live chase still require this game test.
No native branch or offset was added. Manual shopping remains deferred.

Result confirmation: match 33 references replay IDs 882, 883 and 884. Their seeds,
final ticks, controlled athlete/player/champion and KDA match the three original
workers. The recorded series is your team's 2–1 win. This verifies the recorded
series; disk save reload persistence was not separately tested.
