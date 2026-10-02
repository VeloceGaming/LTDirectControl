# Twenty-third test: consecutive battles and save reload

Launch once with Harbinger disabled. The confirmed 0.22 package is preserved in
`dist/backups/0.22.0`. Keep the game running throughout these checks.

1. **First battle:** start control normally. Check movement, attack-move,
   Q/W/R, recall, camera and icons briefly. Hovering skills/items should no
   longer show the placeholder popup; held Tab still shows team information.
2. **Next set:** finish the battle, or Return to AI and let it finish. Continue
   through the game's result/feedback and draft screens. The next battlefield
   should stop at its beginning and offer Start control again. It must resolve
   the newly drafted champion and your team's actual side. Before that battle,
   Ctrl+1..5 should allow choosing your lane again; the last lane is the default.
3. **Clean state:** enable champion-only targeting, use camera lock and leave
   an aiming display open before handing the first battle to AI. The next battle
   should begin with targeting off, camera free and no old aiming/order/recall.
   Check Pause/Resume and Return to AI in the second battle. After Return to AI,
   Start must not regain control of that same battle, including in its replay.
4. **Another match:** progress to another scheduled match without restarting.
   It should prepare and start control the same way, with the correct own player.
5. **Save reload:** return to the title screen without quitting, then load a
   different save and enter a match. Ownership must come from that save. If
   convenient, also reload the original pre-match save: repeating the same
   match/seed must work after a title transition.

Please report whether the second set, another match and the save reload each
offered Start control and selected the correct champion/team. If a transition
fails, stop there and report the displayed message and which screen preceded it.
There is no need to replay all previously confirmed controls in every match.

Validation: 91 probe tests and 18 core tests pass; both packages pass all-target
Clippy with warnings denied and format checks. Tests cover next-battle binding,
retired-battle rejection, old publication waits, save reload, ownership refresh,
command/metadata/HUD resets and held input edges. Native multi-battle transitions
still require this test. Startup/READY/heartbeat recovery guards remain.

Result evidence now uses seed and session generation in filenames, so repeated
saved battles in the same process do not overwrite each other. The prior result
poll continues between battles until another is bound, and clears at title.
Saved-result authority remains unverified.
