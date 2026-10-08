# Adaptive inventory and styled Tab, 0.47.0

The user accepted all 0.46.0 fixes and the Tab design, asked to remove the team-name stripe, described vanilla four-slot and Riot six-slot environments plus possible five-slot mods, and explicitly authorized direct implementation without another HTML approval round.

## Capacity source

The existing reviewed native reader copies each player's full automatic-purchase target vector from fields +0x350/+0x358/+0x360. These fields have native buying-consumer instruction guards in the 0.6.3 profile. This vector already feeds the accepted purchase forecast. Its length, when nonempty/valid, drives presentation capacity. Catalogue size and enabled mod identity are not used. Owned count is a lower bound, so inconsistent/absent plans cannot hide observed items. A missing read retains knowledge only for the same player/match; unknown capacity renders observed items without a false full-build declaration.

This is capacity for the game's active automatic build. A future mod permitting manual purchases outside a shorter target vector may expose a distinct maximum; that would require a dedicated consumer-backed reader. This pass does not assume list length proves such a separate maximum. Synthetic four/five/six data tests cover layout; only the user's current mod environment has been tested in-game historically.

## HUD

Keep existing HP/skills, strip, gold and next-purchase positions. Inventory is right-aligned at the existing edge, using 36px tiles and 32px artwork. Four/five/six slots narrow the group without shrinking icons or moving other HUD groups. Actual tooltip hit areas use the same layout function. The purchase tile uses a separate sentinel so seventh+ item indices cannot masquerade as next purchase. Additional item nodes are spawned only above six; extra rows are blocked from battlefield clicks. All image/property updates retain caching.

## Tab

Native replacement uses the approved warm gray, packaged numeric font, 22px KDA, 21px CS/gold, six-by-default 32px tiles and portrait availability, with no team-name stripe. Side borders distinguish native blue/red; selected row has a thin yellow edge. Inventory column width follows the largest player capacity and remains aligned for all ten rows. Each row shows its own capacity. Additional columns/rows are allocated as needed, rather than truncating to six.

Live SDK scalars and owned item keys are copied at the existing six-tick roster sampling cadence; UI updates remain field/text cached. Data stays on the bound worker/match. Dead identity persists across absent entities. Stale running samples show dashes for numeric data and suppress countdown extrapolation; paused native readings remain frozen. Item artwork reuses the HUD's accepted registered-item metadata and logical overridden sheet, so active mod icon mappings are preserved. The native spectator panel stays hidden during control and is restored through the existing takeover mechanism on release.

Native font/face fitting, rich-label rendering and actual four/five/six environments need user testing. No automated check claims in-game rendering.


Verification: 213 probe +18 core tests passed, core/probe Clippy with warnings denied, formatting and release build passed. Exact 0.6.3 identity, 148 native anchors and 17 layout guards verified. ABI6/null-host check, 75 unchanged packaged UI/font assets and 77-file ZIP verified. Installed files match the verified fingerprints. Client clones the full roster only while Tab is open. Native rendering/gameplay remains a user test.
