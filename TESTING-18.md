# Eighteenth test: HUD draw order, size and large hover popup

Fresh launch with Harbinger disabled. Select your lane before the match, check
your champion/side at READY, then Ctrl+Home. Ctrl+End releases. The existing
**60-second running guard** remains. Both the previous 0.17 package and the
confirmed 0.16 package are preserved in dist/backups.

1. **HUD readability and size:** at READY and during control, portrait, skill
   icons, key labels, health, gold and stats should be clearly visible above
   the dark background. The panel is now 600 x 120 UI units. Check that nothing
   overlaps and that the minimap remains usable. Lancer is a useful first check
   because its native icon references are known; unknown custom champions still
   use the existing fallback.
2. **Large popup:** hover the same places that previously opened the large
   champion skill-information popup, including hidden portrait/team entries.
   It should stay hidden while control is bound. The log now records whether
   `ingame.champion_info_tooltip` exists and up to four suppression results.
3. **Own HUD interaction:** hover Q/W/R and an occupied item slot. Their brief
   hints should remain readable; empty slots/gaps should not open hints.
   Right-clicks, cast confirmation, middle-drag starts and zoom over the HUD
   must remain blocked. Check movement/camera immediately outside its smaller
   boundaries and centered hold-Tab right-click-through.
4. **Release:** Ctrl+End should hide this HUD and restore spectator panels and
   normal portrait hover. The cleanup stops suppressing native popups on release;
   it does not force an old popup to reopen. Casting, recall and camera controls
   otherwise remain as before.

Please report HUD readability, the large popup behavior and restoration after
release. A screenshot helps if any of these still fail. Automated checks cover
control logic and hit geometry; they cannot establish visible native rendering
or the game's hover timing.
