# Investigation: how far a line hit reaches (2026-10-22)

The question (the author, playing 1.5.2-dev3): Aatrox Q from "Silver's
League of Legend Reborn" (`test_mod_aatrox`, `LineRangeProjectile`, `width`
11000, `length` 32000, cast in a direction, range 32) seems to hit about twice
its `length` away.

## Findings

**Read from the code (0.6.4 `0x1705730`, 0.6.5 `0x1537f40`).** A line hit
(`LineRangeProjectile` in champion files; the live reader's "Swordman ult"
family) builds the game's segment shape (tag 1) from the caster to `length`
ahead in the aimed direction (cut at the map's edge), with `width` as the
segment's radius. Recorded in 1.5.2-dev1.

**Read from the code (0.6.4 `0x13e9a20`; the same instructions at 0.6.5
`0x130b6db` and 0.7.0 Beta 2 `0x17d892b`).** The game's shared shape check
(tag 0 circle, 1 segment, 2 rectangle, 3 cone; called by the hit sites listed
in `investigation-stat-rules-0.6.4.md` with the target's position and its
reach) tests a segment like this:

```text
t = clamp(((p - from) . (to - from)) / |to - from|^2, 0, 1)   (in 1/10000)
hit if |p - (from + t (to - from))|^2 <= (reach + radius)^2
```

`p` is the target's centre; `reach` is the target's body radius (about 10
units for a champion), plus the stat rules' margin when they apply. So a line
hit reaches every point within `width` of the segment: a capsule with rounded
ends, `width` behind the caster and `width` past the end, plus the target's
body. The same check confirms the 0.6.2 finding in
`docs/history/preview-geometry-investigation.md`: the line's width is a
distance from its centre line, not a diameter.

Circles and rectangles also add the target's body (tags 0 and 2). The mod's
previews draw each shape at its own size, without the body, for every shape;
that stays.

## What it means

- Aatrox Q: a target's centre is hit up to 32 + 11 = 43 units ahead, so a
  champion whose body reaches about 53 to 63 units can be hit. That is the
  author's "about twice the length" (64); it is not a rule of 2 x `length`.
- Bulldozer Q (`width` 60000, `length` 30000): a stadium 120 wide reaching 60
  behind Bulldozer and 90 ahead, not the 120 x 30 box drawn in 1.5.2.
- Every line hit (about 70 Workshop skills and base Swordman R) is drawn
  without its rounded ends today.

Not seen in game: the rounded ends themselves.

**Superseded 2026-10-22 (the author):** the "about twice the length" came
from Aatrox Q's visual effect, which is drawn larger than the hit; the range
in play is correct. The author's decision: no change to how line hits are
drawn. The code finding above stands as read.

**Superseded 2026-10-23 (the author, playing 1.5.2 with "Show hit areas" on):**
Taliyah R (`cf_taliyah`, `width` 25000, `length` 150000) hit a jungle monster
just past the drawn end of the line, while Taliyah Q (a projectile of radius
15) missed one at about the same distance: seen in game, consistent with the
rounded ends read above. The author's decision: line hits have rounded ends
by default (`Shape::Capsule`, 1.5.2), and a piece can turn them off. The
target's body is still not drawn, as for every shape.

## Projectiles (2026-10-23)

The question (the author): does Taliyah Q (`cf_taliyah`, a `LinearProjectile`
of radius 15000, range 100000, stopping at the first unit) follow the same
rule? **Read from the code (0.6.4):**

- The launch (`0x1a6e7b0`) starts the projectile at the caster and computes
  its end `range` ahead in the aimed direction (cut at the map's edge), then
  hands it to the world (the same method the line hit uses).
- The projectile update (`0x13e25a0`) runs every tick. For a moving
  projectile it counts down a flight counter (`+0x70`); while ticks remain,
  it moves the projectile one step and then calls the projectile hit
  dispatcher (`0x13da020`). On the tick the counter reaches zero it marks the
  projectile finished (`+0x130`) **and still runs the hit check**, at the
  position it last moved to.
- The hit functions the dispatcher reaches (for example `0x1044f60`) test
  the projectile's own shape, a circle of its radius, centred on its current
  position (`+0x100`, `+0x108`), with the shared shape check: hit if the
  target's centre is within the radius plus the target's reach (its body,
  plus the stat rules' margin).

So a projectile sweeps a capsule: it reaches its radius past its last
position (within one step, `speed` per tick, of the full range: Taliyah Q
4.2) and, after its first step, its radius minus one step behind the caster.
Projectiles drawn as flat lines are short by about their radius at the end.

Not read: how the flight counter is set (so the last position may stop up to
one step short of `range`), and whether the launch's `y_offset - 5` start
(`0x1a6eb87`) moves the hit position or only the picture. Not seen in game:
a projectile hitting past its drawn end (the author's Taliyah Q test missed a
target further out than its radius, which agrees).
