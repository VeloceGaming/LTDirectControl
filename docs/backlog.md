# Backlog

What is left to do, as of 0.80.0 (2026-10-10). Finished and declined items
are listed at the end for reference.

## Open

1. **Match freeze (0.77.2, once).** The worker stopped mid-match with
   nothing logged. 0.77.3 logs `WORKER STALL ... last_step=...` (always
   written); if it recurs, the line names where the worker stopped.
2. **Skill previews.** 0.75.2 decodes Ice Mage R (cone), Bard R (aura) and
   Exorcist R (area at the cast), matched to `champion_info`; Gunner Q/R/W,
   Bard Q, Exorcist W and Executioner W are single-target or self buffs and
   correctly show no area. 0.75.3: Dancer R as a fan of 4-10 blades from
   her kill stack; Cavalry Knight R (a speed buff and road) shows no
   direction guide. Bard W (changed by the 0.74.1 landing rule) is untested in game.
   The user keeps observing every champion's preview over time.
   Known limits, accepted for now: buff-dependent skills (SwitchByBuff)
   show the unbuffed version; dashes that stop at the first unit (Rush) have
   no known end; timed dashes are drawn one body wide.
   Keep the PREVIEW TREE / PREVIEW ROSTER logging until previews are done.
3. **Ability tooltip numbers ("…").** Native descriptions are enabled for
   all champions since 0.69.1, with the old resolver as fallback. Of 204
   localized entries, 6 are confirmed in logs and 198 are unobserved. Needs
   broader in-game testing. See
   [investigation-tooltips-native.md](investigation-tooltips-native.md) and
   `tools/audit_tooltips.py`.
4. **Emotes and performance validation.** The user accepted the built-in
   wheel and display controls (0.79.0–0.79.1). Static PNG import/library
   assignment is implemented in 0.80.0; native imported texture loading
   after restart still needs user testing. Custom sounds and animated
   imports are deferred. Detailed performance capture/reporting is available;
   lower-spec hardware and extended compatibility runs remain outstanding.
   See [emotes-and-performance.md](emotes-and-performance.md).
5. **Whole build plan from the start (0.78.0-0.78.1).** 0.78.0 log: the decision grows the plan 4 -> 6, nothing bought; 0.78.1 opens the shop one tick later so Recommended shows it. Manual shopping
   skipped the game's buy-new decision, where the AI / Riot complete the plan
   (slots 5-6). 0.78.0 runs that decision once, discarding its answer, and
   opens the shop when done. Confirm `SHOP DRY RUN ... 4 -> 6` and no
   unexpected purchase.
6. **Stats panel values from items.** Haste, lifesteal and penetration are
   summed from buffs; check them against the game's own panel with items
   that grant them (`STATS own raw` lines, every 10 s).
7. **UI refactor (later, when the user is motivated).** Option 1 of the
   customisation discussion: let modders restyle the HUD without editing
   Rust templates. The background colour (0.76-0.77.2) is the built-in
   limited customisation meanwhile.
8. **Public release.** Rename the mod ID and finish the Steam description.

## Done

- Stats panels (0.77.0-0.77.3, accepted): own stats left of the Q slot
  (C toggles), League-style left-click selection with a target frame
  (face or tower / jungle / minion glyph, health and shield, 12 stats).
- Background colour (0.76.4-0.77.2): one setting moves every surface,
  frame and hover grey by the same offset (`ui_theme`); text inks fixed.
  Stat icons (game sheet plus the mod's sprite sheet) also in shop filters.
- Skill preview redesign (0.75.0-0.75.3, accepted) from the user's design
  (kept outside git); the 0.75 Drawing test setting was
  removed after acceptance (0.77.3 housekeeping).

- Selection pass (0.72.0-0.72.3, accepted): bodies measured from the art,
  placed with the game's draw data; the agreed League-style ranking.
- Skill previews (0.71-0.74.3): 13 native effect types decoded; landing
  and post-dash placement rules.
- Testing aid (0.74.1-0.74.3): Home+End, no cooldowns (+900 haste).
- F11 also pauses (0.67.0).
- Shop "Vanilla order" option, default on (0.67.0).

## Declined

- The design's 550 ms dash reveal (user, 2026-10-09: "means little and can
  backfire").

- Esc on the shop also opens the game's Esc menu (user, 2026-10-09: too
  much work for a small thing). The SDK cannot consume keys and the game
  opens the menu through a path not yet found.
