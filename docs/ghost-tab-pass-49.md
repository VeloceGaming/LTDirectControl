# Ghost tooltip and Tab pass (0.49.0)

The installed 0.6.3 bundle contains asset/base/setting/champion_info (champion_info_sheet). Ghost Q is skill1, while the previous normalized tooltip metadata expected skill. Its takedown values are outside the Q block: add_attack=5, add_attack_speed=1, heal=200. The source fragment is recorded in research/ghost-tooltip-source-49.json. The base tooltip entry now includes the actual Q block and these parent fields. skill_spec handles the skill/skill1 alias and exposes Ghost's parent bonus fields to description resolution; custom declaration values take precedence over the bundled entry as before.

The native localization uses Time for Ghost W's number of uses. The formatter maps that placeholder to UseCount only for Ghost W, reading charge_count=3. Other champions' Time and Ghost R's duration resolution are unaffected. Tests vary the input bonus and charge values to ensure these are data-derived rather than hardcoded numeric replacements. Unknown fields remain unknown.

Cooldown/range glyphs move upward four pixels to match the numeric glyphs observed in user screenshots. Static template and dynamic properties agree; metadata lookup shares skill_spec so Q's aliased block is also available to cooldown/range display. Native alignment requires visual verification.

TeamUi no longer accepts a pointer, interpolates row hover, or returns input-blocking rectangles. Every native panel node remains ignore_event. The existing battlefield command path therefore handles clicks through the panel, including native target picking; ordinary HUD/menu bounds are retained. The controlled-row indicator is static. Native gameplay verification remains the user's task.
