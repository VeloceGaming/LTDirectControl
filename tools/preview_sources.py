"""Where each preview size comes from (step 4 of the agreed preview plan,
stage A; docs/backlog.md item 2).

The mod is to draw every skill's shape from a table shipped inside it and
read its sizes from the game's own numbers by name, which follow the
player's database edits and the save's balance patches. This module decides,
for each size the preview draws, which of the skill's numbers it is:

- `{"name": "attack_range", "recorded": 62}`: the game's number divided by
  1000 (distances are kept in thousandths); `"times": 0.5` or `2` when a
  width is half or twice that number;
- `{"name": "projectile_count", "unit": "count"}` or `"unit": "degrees"`;
- `{"cast_range": true}`: the live cast range (it follows level, patches
  and bonuses);
- `{"fixed": 12}`: a number the game does not hold by name (projectile
  widths in code, Hammerer W's forward offset).

It learns from the pairs the mod records (`numbers` in
research/skill-shapes.json: each skill's numbers by name, its live cast
range and the shapes the live reader read at the same moment), so a size and
its number always come from the same match. A size counts as a number only
when that number is a distance in the game's patch rules (or one of the
common size names asked of skills without rules) and no other such number
has the same value; when two do, the author picks in the viewer
(`skill-sources.json`).

Used by tools/preview_viewer.py (shown and edited per skill) and
tools/build_preview_table.py (the shipped table). Reads nothing itself.
"""

DISTANCE, COUNT, DEGREES = 'distance', 'count', 'degrees'
# Sizes asked of skills whose patch name has no rules or no sizes
# (probe/src/patch_fields.rs `COMMON`, without the cooldown).
COMMON = {
    'range': DISTANCE, 'attack_range': DISTANCE, 'radius': DISTANCE, 'width': DISTANCE,
    'line_width': DISTANCE, 'projectile_radius': DISTANCE, 'explosion_range': DISTANCE,
    'area_range': DISTANCE, 'buff_range': DISTANCE, 'heal_range': DISTANCE,
    'half_angle_deg': DEGREES, 'fan_angle': DEGREES, 'projectile_count': COUNT,
    'move_distance': DISTANCE, 'move_range': DISTANCE,
}
# Within this much (world units) a size equals a number.
TOLERANCE = 0.006
# How a size relates to a number, by its role: d a distance, w half a
# line's width (a radius, or half a full width), W a full width (twice a
# radius, or a width), l a length (the cast range first), c a count, a an
# angle in degrees.
TIMES = {'d': (1,), 'w': (1, 0.5), 'W': (1, 2), 'l': (1,)}
# The sizes of each recorded shape kind, with their roles.
SHAPE_SIZES = {
    'circle': [('radius', 'd')],
    'strip': [('radius', 'w'), ('length', 'l')],
    'shot': [('radius', 'w'), ('length', 'l')],
    'rectangle': [('width', 'd'), ('height', 'd')],
    'cone': [('radius', 'd')],
    'spread': [('count', 'c'), ('radius', 'w'), ('length', 'l'), ('degrees', 'a')],
    'fan': [('radius', 'w'), ('length', 'l')],
    'reach': [('extra', 'd')],
    'backstep': [('length', 'l')],
}
PLACEMENT_SIZES = {'forward': 'd', 'end': 'l', 'landing': 'l'}
# The numbers of each assignment type (tools/preview_viewer.py `REQUIRED`).
ASSIGNED_SIZES = {
    'area_radius': 'd', 'radius': 'd', 'depth': 'd', 'length': 'l', 'distance': 'l',
    'count': 'c', 'spread': 'a', 'angle': 'a', 'extra': 'd',
}


def units(rules, kind, values):
    """Name -> unit for the numbers a skill answered that can be sizes."""
    actions = (rules or {}).get('actions') or {}
    fields = actions.get(kind)
    out = {}
    for name in values:
        if fields is not None and name in fields:
            rule = fields[name] or {}
            if rule.get('format') == 'distance':
                out[name] = DISTANCE
            elif name in COMMON and COMMON[name] != DISTANCE:
                out[name] = COMMON[name]
        elif name in COMMON:
            # Asked as a common size name (Workshop and unknown skills).
            out[name] = COMMON[name]
    return out


def candidates(size, role, values, unit_of, cast_range):
    """Every source that gives `size` for a size of this role."""
    if role == 'l' and cast_range and abs(size - cast_range) <= TOLERANCE:
        return [{'cast_range': True}]
    found = []
    for name, number in sorted(values.items()):
        unit = unit_of.get(name)
        if not isinstance(number, (int, float)) or isinstance(number, bool):
            continue
        if role == 'c':
            if unit == COUNT and number == size:
                found.append({'name': name, 'unit': COUNT})
        elif role == 'a':
            if unit == DEGREES and abs(number - size) <= 0.01:
                found.append({'name': name, 'unit': DEGREES})
        elif unit == DISTANCE:
            for times in TIMES[role]:
                if abs(number / 1000 * times - size) <= TOLERANCE:
                    found.append({'name': name, **({'times': times} if times != 1 else {})})
                    break
    return found


def valid_pick(pick, values):
    if not isinstance(pick, dict):
        return False
    if pick.get('cast_range') is True or isinstance(pick.get('fixed'), (int, float)):
        return True
    return isinstance(pick.get('name'), str) and pick['name'] in values


def still_fits(pick, found):
    """A pick of one of the game's numbers holds only while that number
    still gives the size (1.5.2-dev1: Bulldozer Q's pick `width x 0.5`
    stopped fitting when its recorded radius was corrected to the full
    `width`). A fixed number or the cast range always holds."""
    if 'name' not in pick:
        return True
    key = lambda c: (c.get('name'), c.get('times', 1), c.get('unit'))
    return key(pick) in {key(c) for c in found}


def decide(key, size, role, pair, unit_of, picks):
    """One size: its source, how sure, and the other numbers that fit."""
    entry = {'key': key, 'value': size}
    if size is None or not isinstance(size, (int, float)):
        return None
    if size == 0:
        # Zero is "none" or "the cast range" in the mod's own shapes.
        return {**entry, 'source': {'fixed': 0}, 'status': 'fixed', 'candidates': []}
    if pair is None:
        return {**entry, 'source': {'fixed': size}, 'status': 'no_numbers', 'candidates': []}
    values = pair.get('values') or {}
    found = candidates(size, role, values, unit_of, pair.get('cast_range'))
    entry['candidates'] = found
    pick = (picks or {}).get(key)
    if valid_pick(pick, values) and still_fits(pick, found):
        source = dict(pick)
        status = 'picked'
    elif len(found) == 1:
        source = dict(found[0])
        status = 'cast_range' if source.get('cast_range') else 'named'
    elif found:
        source, status = {'fixed': size}, 'pick'
    else:
        source, status = {'fixed': size}, 'fixed'
    if 'fixed' not in source:
        source['recorded'] = size
    return {**entry, 'source': source, 'status': status}


def landed(shapes):
    """Shapes recorded before 1.5.0-dev19 with the rule the readers now
    apply: a circle around the caster that goes with a move to the aim (or
    onto the target unit) is around where the caster lands (the author,
    2026-10-21: Pole Warrior W, Invincible R). A line or cone keeps its
    place. Shapes recorded since already come this way."""
    moves = any(s.get('kind') in ('dash', 'blink') and s.get('at') == 'aim' for s in shapes)
    if not moves:
        return shapes
    return [dict(s, at='aim') if s.get('kind') == 'circle' and s.get('at') == 'caster' else s
            for s in shapes]


# Champions whose piercing shots come from the game's champion-skill effect
# (probe/src/native_adapter/windows/preview.rs, CHAMPION_SKILL): their radius
# was always the hit radius.
CHAMPION_SKILL = {'bombardier', 'ferryman', 'gladiator', 'hypnotist'}


def widened(shapes, champion):
    """Shots recorded before 1.5.2-dev1 with the line hit's true width.

    Read from the game's code (0.6.4 and 0.6.5, 1.5.2-dev1): the game's
    line hit (`LineRangeProjectile` in data files: Bulldozer Q, Abbie W;
    Swordman R and other base skills) hits within its `width` on each side
    of the line. The live reader halved it until 1.5.2-dev1, so a piercing
    shot without `hit_radius` holds half the radius. Doubled here, marked,
    so a second pass changes nothing. Shapes recorded since carry the mark."""
    if champion in CHAMPION_SKILL:
        return shapes
    return [dict(s, radius=s['radius'] * 2, hit_radius=True)
            if s.get('kind') == 'shot' and s.get('pierce') is True and not s.get('hit_radius')
            and isinstance(s.get('radius'), (int, float)) else s
            for s in shapes]


def shape_sizes(shapes, pair, unit_of, picks, prefix=''):
    sizes = []
    for index, shape in enumerate(shapes):
        for field, role in SHAPE_SIZES.get(shape.get('kind'), []):
            d = decide(f'{prefix}{index}.{field}', shape.get(field), role, pair, unit_of, picks)
            if d:
                sizes.append(d)
        at = shape.get('at')
        if isinstance(at, dict):
            for place, role in PLACEMENT_SIZES.items():
                if place in at:
                    d = decide(f'{prefix}{index}.at.{place}', at[place], role, pair, unit_of, picks)
                    if d:
                        sizes.append(d)
    return sizes


def assigned_sizes(assignment, pair, unit_of, picks):
    sizes = []
    for field, value in assignment.items():
        role = ASSIGNED_SIZES.get(field)
        if field == 'width':
            role = 'W' if assignment.get('type') in ('line', 'fan') else 'd'
        if role is None:
            continue
        d = decide(f'assigned.{field}', value, role, pair, unit_of, picks)
        if d:
            sizes.append(d)
    return sizes


def skill(champion, slot, table, rules, assignments, picks):
    """What the mod would draw for one skill and where each size comes from:
    {from, shapes|assigned, sizes}. `from` follows the mod's order: the
    author's assignment, then the live reader, then the champion file."""
    recorded = ((table.get('champions') or {}).get(champion) or {}).get(slot) or {}
    pair = ((table.get('numbers') or {}).get(champion) or {}).get(slot)
    values = (pair or {}).get('values') or {}
    unit_of = units(rules, (pair or {}).get('kind'), values)
    mine = (picks or {}).get(f'{champion} {slot}') or {}
    cast = recorded.get('cast') or {}
    out = {'kind': (pair or {}).get('kind'), 'has_numbers': pair is not None}
    live = landed(widened((pair or {}).get('live') or recorded.get('shapes') or [], champion))
    file = landed((recorded.get('file') or {}).get('shapes') or [])
    assignment = (assignments or {}).get(f'{champion} {slot}')
    if isinstance(assignment, dict) and assignment.get('type') == 'custom':
        # The author's own pieces (1.5.2-dev2): each size already names its
        # source; shipped as written.
        pieces = assignment.get('pieces')
        return {**out, 'from': 'custom', 'pieces': pieces if isinstance(pieces, list) else [],
                'sizes': []}
    if isinstance(assignment, dict) and assignment.get('type'):
        out.update({'from': 'assigned', 'assigned': assignment,
                    'sizes': assigned_sizes(assignment, pair, unit_of, mine)})
        if assignment.get('add'):
            under = live or file
            out['shapes'] = under
            out['sizes'] += shape_sizes(under, pair, unit_of, mine)
        return out
    casting = cast.get('casting')
    if casting is not None and not 0 <= casting <= 3:
        return {**out, 'from': 'passive', 'sizes': []}
    if live:
        return {**out, 'from': 'live', 'shapes': live, 'sizes': shape_sizes(live, pair, unit_of, mine)}
    if file:
        return {**out, 'from': 'file', 'shapes': file, 'sizes': shape_sizes(file, pair, unit_of, mine)}
    if recorded.get('no_area') or (recorded.get('file') or {}).get('no_area'):
        return {**out, 'from': 'no_area', 'sizes': []}
    return {**out, 'from': 'none', 'sizes': []}


def all_skills(table, rules, assignments, picks):
    """`skill` for every recorded champion: {champion: {slot: ...}}."""
    return {champion: {slot: skill(champion, slot, table, rules, assignments, picks)
                       for slot in ('Q', 'W', 'R')}
            for champion in sorted(table.get('champions') or {})}


def with_sources(item, sizes, prefix):
    """A shape or assignment with each size replaced by its source."""
    out = {k: (dict(v) if isinstance(v, dict) else v) for k, v in item.items()}
    for size in sizes:
        if not size['key'].startswith(prefix):
            continue
        path = size['key'][len(prefix):].split('.')
        target = out
        for part in path[:-1]:
            target = target[part]
        target[path[-1]] = size['source']
    return out
