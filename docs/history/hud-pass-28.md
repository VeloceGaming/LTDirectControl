# Approved HUD implementation — 0.28.0

The native UI uses the reviewed HTML geometry directly at the game's 1920×1080
UI scale. The combat panel is 224×124 at (848,900), with 64×64 skills, 8-pixel
gaps and an explicit green HP bar. The 56-pixel bottom strip is flat #252525;
the panel is #292929. KDA/CS are horizontal. Only exceptional control failures
produce a status sentence. Names/roles are absent from the playing HUD.

Preparation hides live stats, combat and inventory while preserving the gray
strip. Five 64-pixel sprite choices have a yellow selection border and check.
Selection is bound to the same match/generation as before. Bottom-left controls
use 28-pixel glyphs within 40-pixel buttons; the preparation play button is 48×42.
AI release moves into the playing controls' more popover. Champion-only state
is omitted from the HUD; a controlled cursor remains a separate future feature.

The transparent HUD root does not consume battlefield input. Only opaque HUD
surfaces are included in the existing input exclusion rectangles. The centered
native Tab scoreboard keeps working modded item art and right-click passthrough;
death masks and simulation-tick countdowns overlay its existing champion sprites.
No custom portrait row is permanently displayed.

## Death rendering

Static reverse engineering of the installed 0.6.2 executable identifies
GameView::render's #Game texture composite. Its shader CALL at RVA 0x1fd6412
passes the post_process shader to RenderCommand::shader (0x1c8f60). A redirect
changes only that command's shader name to the game's existing greyscale shader
while the controlled player is dead. The minimap and UI are separate commands.
The original method still owns and consumes the command; no native allocation
or render pointer is retained. The entire executable fingerprint, call bytes
and method prologue gate installation. Respawn, release and session reset clear
the flag. Map dimensions and projection are untouched.

## Read-only purchase tracker

An SDK item-build observer records the engine's item-index/key catalogue and
returns no override. The actual final build is copied from the player's live
native Vec during the existing worker/SDK borrow, after other mods' overrides.
The local SDK wrapper exposes a narrowly documented unsafe inspection callback;
it does not change the host ABI. The adapter verifies the SDK SimVtable size/getter pointer,
gold-getter bytes and the current buying function's Vec reads before using it.
No host/player pointer survives the callback.

Live ItemSetting data supplies keys, incremental prices, tiers, icons and upgrade
edges. Forecasting completes the last regular inventory slot before advancing
to the next final target. Starter/special tiers are excluded. Fixed paths show
the next item, progress, gold shortfall and sequential affordable upgrades.
Branching paths are deliberately uncertain: the game may choose randomly when
buying. The tooltip lists alternatives; no preview advances the game's RNG or
changes gold, inventory, builds or shopping behavior. Missing metadata produces
an unavailable state. Native user testing must confirm the forecast against
actual buying, especially custom item behavior.

Existing permanent image nodes are retained for all skill/item art, including
modded sheets and PNGs. Forecast recomputation follows gold/build/inventory
changes; it is not repeated every camera/render frame. Original glyph PNGs
contain only grayscale or exact yellow variants. Health and death warning red
are the approved semantic exceptions.

## Remaining work after the HUD game test

Return to input responsiveness and buffering, body/leg picking and ally hover,
accurate skill/item descriptions and dynamic ability previews. This build does
not claim to fix those remaining gameplay issues. An external overlay remains
an architectural option if future native UI limitations justify it.
