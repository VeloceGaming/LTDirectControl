# Selection and registered purchase graph, 0.31.0

## Selection

The previous picker ranked minion hits equally with champion core hits, and ahead
of champion margin hits. Its champion box stopped at the entity position plus
three screen pixels, missing legs in some poses. Minion picking inflated combat
collision radius into a mouse box. These were code findings, not in-game tests.

Champion envelopes now extend four world units sideways, two above, and eight
below the entity origin, plus three screen pixels of forgiveness. Champions win
over other units; within a class, core hits precede margin hits, then body-center
distance and stable ID break ties. Allied champions use identical geometry.

Minions use the largest normal base sprite pose, 25 by 27 source pixels at the
existing half-size world scale, with a small margin. Registered base ingame art
dimensions are also bundled for monsters/structures when their entity name matches.
Unknown monsters have bounded fallback boxes. No combat collision radius or
ability eligibility is changed. Shared picking serves hover, attacks and skills.
The hover path no longer requires an on-map ground coordinate for a visible body.
These are rough envelopes, not per-frame alpha masks; moving-pose fit needs testing.

## Purchase investigation

The item pack's item-builds.json has presets for cf_archangel@mid and jiangshi@mid.
Both exactly match the final builds captured in the user's 0.30 log. Preset choice
was already correct; no duplicate preset-selection system is needed.

Native ItemInfo wrapper construction at 0x2e87840 calls legacy +0x50 into a Vec
cached at object +0x30; legacy +0x58 is cached at +0x48. Getter 0x13aab80 returns
object +0x30 and is bound to ItemInfo vtable +0x80. Native buying 0xe69d20 searches
these forward lists for items that can reach the goal, working backward from it.
The 0.30 reader incorrectly named that list previous_tier and inverted it.

0.31 copies +0x80 as next_tier, validates referenced keys, and derives reverse
links for diagnostics only. No additional native getters or gameplay hooks are
called. Three instruction anchors verify the wrapper mapping alongside the
existing executable fingerprint. Getter +0x50 on the mod-item wrapper returns
true; the earlier suspicion that it filters out all components was not confirmed.

Evidence: research/item-wrapper-31.asm, research/item-cache-31.asm, and
research/items-30.asm. Only owned metadata leaves the AI callback.

Prices and tiers are still live registered values. The existing forecasting
algorithm's assumptions have not been broadly rewritten. Bounded inventory/build
change logs now expose the actual connected component chain and forecast, so
remaining branch/progression differences can be compared with real purchases.
No claim of an accurate native forecast is made before the user's test.
