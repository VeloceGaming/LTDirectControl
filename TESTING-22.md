# Twenty-second test: persistent icons and death camera

Fresh launch with Harbinger disabled. Prior 0.21 is backed up in
`dist/backups/0.21.0`. Use whichever champion is drafted; Circus Blade/Lancer
are useful vanilla checks, and an enabled custom champion tests PNG icons.

1. **Skills:** check Q/W/R art while alive. Unlearned skills should retain art
   but dim until levels 1/3/5. If a custom champion is drafted, its PNG icons
   should appear too. The HUD size and hover behavior should remain unchanged.
2. **Multiple items:** watch at least two occupied item slots. When a new item
   is bought or an item upgrades, the other occupied slots must keep their
   icons. Compare with held Tab. Genuine inventory replacement/consumption can
   change a slot; a purchase must not blank unrelated owned items.
3. **Death:** portrait, skill art, inventory and the native respawn countdown
   should remain. Middle-button dragging and edge pan should work during death,
   including while paused. A drag begun on the battlefield can cross the HUD;
   one begun on the HUD should not pan. Wheel/minimap behavior should remain.
4. **Lock and respawn:** if Y lock is left enabled without deliberate panning,
   it should resume following after respawn. Dragging while dead unlocks it;
   respawn should then leave the camera where you positioned it. Holding Space
   cannot follow an absent champion. Pause/Resume and combat controls remain.

Please report skill art, whether older item icons survive purchases, and death
dragging. If an image is still missing, a screenshot with held Tab for comparison
helps. The log now records permanent image paths, property success, existence
and bounds; these diagnostics do not themselves prove successful drawing.

Validation: 85 probe and 18 core tests pass, with all-target Clippy and format
checks. New tests cover sheet/PNG separation, independent slot updates, absent-
champion camera drag/edge pan, HUD drag exclusion and follow intent on respawn.
Native drawing still requires this game test. Saved-result authority remains
unverified; the 0.21 run reached 11:00 and captured 91 result-screen labels.
