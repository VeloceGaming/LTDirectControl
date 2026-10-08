# 0.33.0 ally hover and monster sizing

## Causes established from current source and installed assets

`PlayerAi::think` constructed populated all-unit data only for the controlled
player, but called `observe_hover_units` unconditionally. Other players and
background callbacks therefore replaced that data with an empty vector.
`target_markers` then fell back to enemy-only data. That lifecycle explains
missing ally rings independently of GPU rendering or low FPS.

Base picking metadata used the maximum dimensions across idle, walking and
attack/skill tags. The installed serpent has idle dimensions up to 59x79 source
pixels, but an attack frame reaches 227x225. Its permanent profile used the
latter. Champion-sized minimum foot padding also inflated small monster bounds.

## Changes

Only the selected living player's owned live-match callback publishes hover
data. The receiver additionally checks match/player identity and living position.
Hover data has a separate timestamp: enemy snapshot freshness cannot hide fresh
allies. A present empty or expired hover snapshot does not fall back to enemies.
Before any hover snapshot exists, the existing fresh enemy fallback remains.
Death, inactive controls, battlefield exit, prepared selection and session reset
clear hover state. A callback proposing an unrelated match does not erase the
current one; actual rearm/reset clears the old state.

The six base jungle monster profiles use idle/run/walk metadata. Changed source
pixel maxima: stump 41x41 -> 37x41; mushroom 47x45 -> 43x45; rhino 71x51 -> 63x51;
serpent 227x225 -> 59x79; bee 33x41 -> 27x41. Epic remains 109x133. Regeneration
confirmed all champion and structure profiles unchanged.

Monster envelopes use half body width +2 world units, body height +2, a foot pad
of 8% of height bounded to 3-6 world units, and 2 UI pixels edge allowance.
Champion and minion formulas are retained. Highlights derive from the same
selection envelope. Combat radius, attack/skill range and native physics do not
change. Unknown monsters keep the existing bounded body fallback, using the
new monster padding rather than champion padding.

## Verification and limits

Regression tests interleave unrelated player/match publications, expire enemy
and hover snapshots independently, exercise empty authoritative snapshots and
death/inactive/session cleanup, and check monster bodies/feet across zooms.
Bundled metadata checks cover the serpent, rhino, stump, epic and unchanged
champion examples. Native hooks, pacing and input command rules are unchanged.

Unit tests cannot establish actual in-game flicker or visible fit. User testing
is pending; see [TESTING-33.md](../TESTING-33.md). FPS profiling, repetitive Tab
logging, cursor work and patch migration remain deferred separately.

156 probe and 18 core tests passed, along with Clippy and formatting checks.
Release DLL entry/null-host checks and all 39 native anchors passed. The ZIP
contains 44 verified matching files. Installed DLL, metadata and 42 UI assets
match the package fingerprints. The prior 0.32.0 installation is preserved in
`dist/backups/0.32.0-before-0.33.0-20261006-161332` (44 files verified).
