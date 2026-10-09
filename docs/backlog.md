# Backlog

What is left to do, as of 0.74.4 (2026-10-09). Finished and declined items
are listed at the end for reference.

## Open

1. **Skill preview redesign.** The user is redesigning the previews from
   [preview-drawing.md](preview-drawing.md). Before building it, a small
   drawing test build should confirm in game what that sheet marks as
   untried: very wide lines, `draw_svg` and `draw_sprite`. Then the preview
   look moves into a layered renderer: each preview piece (area, corridor,
   cone, movement, range ring, target) becomes a list of layers in a style
   file, built to the design.
2. **Skill previews still missing** for base-game skills built from effect
   types no JSON names yet: Gunner Q and R, Bard Q and R, Exorcist R,
   Cavalry Knight R, and an unknown `1ad9600/48` inside Executioner W and the
   Poison Dart Hunter skills. Needs static analysis of the game code, as in
   0.71. Bard W (changed by the 0.74.1 landing rule) is untested in game.
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
4. **Emotes.** The last item of the post-shop investigation
   ([investigation-pass-65.md](investigation-pass-65.md)); not started.
5. **Champion stats (optional).** The game's match HUD already shows AD, AP,
   Armor, MR, attack speed and move speed. Adding stats it does not show
   (ability haste, crit, range, lifesteal, penetration) is optional; the
   SDK can read them.
6. **Public release.** Rename the mod ID and finish the Steam description.

## Done

- Selection pass (0.72.0-0.72.3, accepted): bodies measured from the art,
  placed with the game's draw data; the agreed League-style ranking.
- Skill previews (0.71-0.74.3): 13 native effect types decoded; landing
  and post-dash placement rules.
- Testing aid (0.74.1-0.74.3): Home+End, no cooldowns (+900 haste).
- F11 also pauses (0.67.0).
- Shop "Vanilla order" option, default on (0.67.0).

## Declined

- Esc on the shop also opens the game's Esc menu (user, 2026-10-09: too
  much work for a small thing). The SDK cannot consume keys and the game
  opens the menu through a path not yet found.
