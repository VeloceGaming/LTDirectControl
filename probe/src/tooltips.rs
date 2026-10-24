//! Host-localized descriptions. Unknown formula parameters remain visibly unknown.
use mod_api_stable::{SettingTargetV1, StableClient};
use serde_json::Value;
use std::sync::OnceLock;
pub fn base_champion(name: &str) -> Option<&'static Value> {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("tooltip_assets.json")).expect("tooltip metadata")
    })
    .get(name)
}
pub fn translated(ctx: &StableClient<'_>, reference: &str) -> Option<String> {
    if !reference.starts_with('#') && !reference.starts_with("asset/") {
        return Some(reference.into());
    }
    let key = reference.trim_start_matches('#');
    [key, reference].into_iter().find_map(|key| {
        ctx.i18n(key).filter(|s| {
            !s.is_empty()
                && s != key
                && s != reference
                && !s.starts_with("asset/")
                && !s.starts_with("#asset/")
        })
    })
}
pub fn champion_name(ctx: &StableClient<'_>, champion: &str) -> String {
    translated(
        ctx,
        &format!("#asset/base/text/champion?description.{champion}.name"),
    )
    .unwrap_or_else(|| champion.replace('_', " "))
}
pub fn plain(text: &str) -> String {
    let mut result = String::new();
    let mut markup = false;
    let mut parameter = false;
    for c in text.chars() {
        match c {
            '<' => markup = true,
            '>' if markup => markup = false,
            '{' if !markup => {
                parameter = true;
                result.push('…');
            }
            '}' if parameter => parameter = false,
            _ if !markup && !parameter => result.push(c),
            _ => {}
        }
    }
    result
}
/// Keep the game's RGBA stat-color runs and reset markers. Other markup
/// (including inline asset images) is omitted; unknown values stay unknown.
pub fn rich(text: &str) -> String {
    rich_markup(text, false)
}
/// Item effect text additionally keeps the item mod's own inline stat icons
/// (`<i#asset/…:name>`), which the game's text renderer draws in its own
/// item screens.
pub fn rich_with_icons(text: &str) -> String {
    rich_markup(text, true)
}
fn inline_icon(tag: &str) -> bool {
    tag.strip_prefix("i#asset/").is_some_and(|path| {
        path.contains(':') && path.bytes().all(|c| c.is_ascii_graphic() || c == b' ')
    })
}
fn rich_markup(text: &str, icons: bool) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.starts_with('<') {
            if let Some(end) = remaining.find('>') {
                let tag = &remaining[1..end];
                if tag.is_empty()
                    || (tag.len() == 9
                        && tag.starts_with('#')
                        && tag[1..].bytes().all(|c| c.is_ascii_hexdigit()))
                    || (icons && inline_icon(tag))
                {
                    result.push_str(&remaining[..=end]);
                }
                remaining = &remaining[end + 1..];
                continue;
            }
        }
        if remaining.starts_with('{') {
            if let Some(end) = remaining.find('}') {
                result.push('…');
                remaining = &remaining[end + 1..];
                continue;
            }
        }
        let c = remaining.chars().next().expect("nonempty text");
        result.push(c);
        remaining = &remaining[c.len_utf8()..];
    }
    result
}
/// Skill text with values computed from the champion's current stats, like
/// League: "<#c>60<> + <i#…:ad_0><#c>80% Attack Damage<>" (base + percentage
/// of attack damage; `ap_0` is ability power) becomes "<i#…:ad_0><#c>154<>",
/// and a pure ratio "<i#…:ad_0><#c>攻擊力的200%<>" the percentage of the stat
/// (seen in game 2026-10-20: Crossbowman Q has no base). Only that shape is
/// changed: an attack- or ability-power icon, a colour run right after it
/// holding one percentage (its words go too: in most languages the run is
/// the stat phrase alone; in Russian it also names the damage type), and
/// optionally a plain base number with a plus sign before the icon. Every other value, other stat icons (health,
/// armour, speeds: whose they are is not always clear) and anything
/// unreadable stay as the game wrote them.
pub fn computed(text: &str, attack: f64, magic: f64) -> String {
    // A stat icon and its percentage run at the start of `text`: the icon,
    // the run's colour, the bytes they take and the value.
    fn stat_term(text: &str, attack: f64, magic: f64) -> Option<(&str, &str, usize, f64)> {
        if !text.starts_with("<i#asset/") {
            return None;
        }
        let icon_len = text.find('>')? + 1;
        let icon = &text[..icon_len];
        let stat = if icon.ends_with(":ad_0>") {
            attack
        } else if icon.ends_with(":ap_0>") {
            magic
        } else {
            return None;
        };
        let (run_len, percent) = percent_run(&text[icon_len..])?;
        let color = color_of(&text[icon_len..])?;
        Some((icon, color, icon_len + run_len, percent / 100. * stat))
    }
    let term = |text| stat_term(text, attack, magic);
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<i#asset/") {
        let Some(icon_len) = rest[start..].find('>').map(|n| n + 1) else {
            break;
        };
        let formula = term(&rest[start..]).and_then(|(icon, color, len, value)| {
            // With a base ("60 + …") the base goes into the total; a pure
            // ratio ("200% …") is the percentage of the stat alone.
            // A plus sign with an unreadable base leaves the text alone.
            let (from, base) = if rest[..start].trim_end().ends_with('+') {
                base_before(&rest[..start])?
            } else {
                (start, 0.)
            };
            // Further "+ [stat] N%" terms join the same total (the author,
            // 2026-10-20: "90 + 100% AD + 100% AP" left its AP part as
            // written), one number behind all their icons, like League.
            let (mut icons, mut to, mut total) = (icon.to_owned(), start + len, base + value);
            while let Some(next) = rest[to..]
                .trim_start()
                .strip_prefix('+')
                .map(str::trim_start)
                .and_then(|after| Some((rest.len() - after.len(), term(after)?)))
            {
                let (at, (icon, _, len, value)) = next;
                if !icons.contains(icon) {
                    icons.push_str(icon);
                }
                (to, total) = (at + len, total + value);
            }
            Some((from, to, icons, color, total))
        });
        match formula {
            Some((from, to, icons, color, total)) => {
                out.push_str(&rest[..from]);
                out.push_str(&format!("{icons}<{color}>{}<>", total.round() as i64));
                rest = &rest[to..];
            }
            None => {
                out.push_str(&rest[..start + icon_len]);
                rest = &rest[start + icon_len..];
            }
        }
    }
    out.push_str(rest);
    out
}
/// The `#rrggbbaa` of a colour tag at the start of `text`.
fn color_of(text: &str) -> Option<&str> {
    let tag = text.strip_prefix('<')?.get(..9)?;
    (tag.starts_with('#')
        && tag[1..].bytes().all(|c| c.is_ascii_hexdigit())
        && text.as_bytes().get(10) == Some(&b'>'))
    .then_some(tag)
}
/// Before an icon: "<#c>60<> + " or "60 + ". The byte where the base starts
/// and its value.
fn base_before(text: &str) -> Option<(usize, f64)> {
    let before_plus = text.trim_end().strip_suffix('+')?.trim_end();
    let (number_end, colored) = match before_plus.strip_suffix("<>") {
        Some(inner) => (inner.len(), true),
        None => (before_plus.len(), false),
    };
    let digits = before_plus[..number_end]
        .bytes()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == b'.')
        .count();
    let number_start = number_end - digits;
    let base: f64 = before_plus[number_start..number_end].parse().ok()?;
    if colored {
        let tag_start = number_start.checked_sub(11)?;
        color_of(&before_plus[tag_start..])?;
        Some((tag_start, base))
    } else {
        Some((number_start, base))
    }
}
/// After an icon: a colour run "<#c>…80%…<>" holding exactly one
/// percentage and nothing unresolved. Its length and the percentage.
fn percent_run(text: &str) -> Option<(usize, f64)> {
    color_of(text)?;
    let close = text.find("<>")?;
    let inner = text.get(11..close)?;
    if inner.contains(['{', '…', '<']) || inner.matches('%').count() != 1 {
        return None;
    }
    // "80%", "80 %" (French, with any space) or "%80" (Turkish).
    let at = inner.find('%')?;
    let number = |s: &str| -> Option<f64> { s.parse().ok().filter(|_| !s.is_empty()) };
    let before = inner[..at].trim_end();
    let digits = before
        .bytes()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == b'.')
        .count();
    let after = &inner[at + 1..];
    let lead = after
        .bytes()
        .take_while(|c| c.is_ascii_digit() || *c == b'.')
        .count();
    let percent = number(&before[before.len() - digits..]).or_else(|| number(&after[..lead]))?;
    Some((close + 2, percent))
}
/// Only declared fields are substituted. Conflicting nested effects remain
/// unknown; animation duration is never used as buff/CC duration.
fn field(spec: &Value, names: &[&str]) -> Option<f64> {
    for name in names {
        if let Some(n) = spec.get(*name).and_then(Value::as_f64) {
            return Some(n);
        }
    }
    fn collect(v: &Value, names: &[&str], out: &mut Vec<f64>, depth: usize) {
        if depth > 20 {
            return;
        }
        match v {
            Value::Object(map) => {
                for name in names {
                    if let Some(n) = map.get(*name).and_then(Value::as_f64) {
                        out.push(n);
                        break;
                    }
                }
                for child in map.values().filter(|v| v.is_object() || v.is_array()) {
                    collect(child, names, out, depth + 1);
                }
            }
            Value::Array(list) => {
                for child in list {
                    collect(child, names, out, depth + 1);
                }
            }
            _ => {}
        }
    }
    let mut values = Vec::new();
    collect(spec, names, &mut values, 0);
    let first = *values.first()?;
    values.iter().all(|n| *n == first).then_some(first)
}
fn parameter(spec: &Value, name: &str, context: &str) -> Option<String> {
    let (names, divisor): (&[&str], f64) = match name {
        "Damage" | "LinkDamage" => (&["damage", "attack"], 1.),
        "Coef" | "Value" => (
            &[
                "attack_ratio",
                "ap_ratio",
                "magic_ratio",
                "heal_attack_ratio",
            ],
            1.,
        ),
        "Heal" => (&["heal"], 1.),
        "HealCoef" => (&["heal_ratio", "heal_attack_ratio", "heal_ap_ratio"], 1.),
        "Shield" => (&["shield"], 1.),
        "ShieldCoef" => (&["shield_ratio", "shield_ap_ratio"], 1.),
        "Slow" => (&["slow", "slow_ratio", "slow_speed"], 1.),
        "SlowTime" => (&["slow_duration"], 60.),
        "Time" => (
            &[
                "bind_duration",
                "stun_duration",
                "airborne_time",
                "airborne",
                "fear_tick",
                "unhealable_time",
                "buff_duration",
                "slow_duration",
            ],
            60.,
        ),
        "Duration" => (
            &[
                "buff_duration",
                "effect_duration",
                "channel_duration",
                "debuff_duration",
                "barrier_tick",
            ],
            60.,
        ),
        "Stun" => (&["stun", "stun_duration"], 60.),
        "Delay" => (&["delay", "effect_delay"], 60.),
        "MarkTime" => (&["mark_duration"], 60.),
        "StealthTime" => (&["invisible_duration"], 60.),
        "BackstepTime" => (&["backstep_tick"], 60.),
        "ShieldTime" => (&["shield_duration"], 60.),
        "SelfStun" => (&["self_stun"], 60.),
        "LinkDuration" => (&["link_duration"], 60.),
        "LinkRange" => (&["link_range"], 1000.),
        "BlockTime" => (&["block_duration"], 60.),
        "Range" => (&["range"], 1000.),
        "Radius" => (&["radius"], 1000.),
        "Count" => (
            &["total_shots", "hit_count", "attack_count", "base_count"],
            1.,
        ),
        "UseCount" => (&["cooltime_use_count", "charge_count"], 1.),
        "MaxCount" => (&["max_count"], 1.),
        "Attack" => (&["add_attack", "attack_boost", "attack"], 1.),
        "AttackRatio" => (&["magic_ratio"], 1.),
        "AttackSpeed" => (
            &["add_attack_speed", "attack_speed_boost", "attack_speed"],
            1.,
        ),
        "MoveSpeed" => (&["move_speed"], 1.),
        "MagicPower" => (&["magic_power_boost", "magic_power"], 1.),
        "Defense" => (&["defence"], 1.),
        "DefenceReduce" => (&["defence_reduce"], 1.),
        "MagicResistanceReduce" => (&["magic_resistance_reduce"], 1.),
        "CoolReduce" => (
            &[
                "skill_cooldown_reduce",
                "cooltime_reduce",
                "cooldown_reduce",
            ],
            1.,
        ),
        "Vamp" => (&["vamp"], 1.),
        "DotDamage" => (&["dot_damage"], 1.),
        "DotCoef" => (&["dot_attack_ratio"], 1.),
        "BonusDamage" => (&["bonus_damage", "damage_per_buff"], 1.),
        "BonusCoef" => (&["damage_per_buff_ratio"], 1.),
        "MarkCoef" => (&["mark_bonus_attack_ratio"], 1.),
        "ReturnDamage" => (&["return_attack"], 1.),
        "ReturnCoef" => (&["return_attack_ratio"], 1.),
        "SelfHeal" => (&["heal_self"], 1.),
        "SelfHealCoef" => (&["heal_self_ratio"], 1.),
        "HpRatio" => (&["max_hp_ratio"], 1.),
        "HpCoef" => (&["missing_hp_ratio"], 1.),
        "ShareRatio" => (&["damage_share_ratio"], 1.),
        "SpreadRatio" => (&["damage_spread_ratio"], 1.),
        "Increase" => (&["damage_increase_per_bounce"], 1.),
        "MaxBounce" => (&["max_bounce"], 1.),
        "Amplify" => (&["amplify"], 1.),
        "ProjectileCount" => (&["projectile_count"], 1.),
        "TensionSlow" => (&["tension_slow_ratio"], 1.),
        _ => return None,
    };
    let names = if matches!(name, "Coef" | "Value") {
        let ad = context.rfind("ad_0");
        let ap = context.rfind("ap_0");
        match (ad, ap) {
            (_, Some(ap)) if ad.is_none_or(|ad| ap > ad) => {
                &["ap_ratio", "magic_ratio", "heal_ap_ratio"][..]
            }
            (Some(_), _) => &["attack_ratio", "heal_attack_ratio"][..],
            _ => names,
        }
    } else {
        names
    };
    let number = field(spec, names)? / divisor;
    if !number.is_finite() {
        return None;
    }
    Some(
        format!("{number:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned(),
    )
}
pub fn resolve(text: &str, spec: Option<&Value>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return rich(&out);
        };
        let value = spec.and_then(|s| parameter(s, &rest[start + 1..start + end], &out));
        out.push_str(value.as_deref().unwrap_or("…"));
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    rich(&out)
}
fn skill_definition(slot: usize, spec: &Value) -> Option<&Value> {
    let tag = ["skill", "skill2", "ult"][slot];
    spec.get(tag)
        .or_else(|| (slot == 0).then(|| spec.get("skill1")).flatten())
}
pub fn skill_spec(champion: &str, slot: usize, spec: &Value) -> Option<Value> {
    let mut value = skill_definition(slot, spec)?.clone();
    // Ghost's takedown bonuses are declared on the champion, outside Q.
    if champion == "ghost" && slot == 0 && value.is_object() {
        for key in ["add_attack", "add_attack_speed", "heal"] {
            if let Some(n) = spec.get(key) {
                value[key] = n.clone();
            }
        }
    }
    Some(value)
}
fn resolve_skill(text: &str, champion: &str, slot: usize, spec: Option<&Value>) -> String {
    // Native Ghost W uses {Time} for charge count, not elapsed seconds.
    if champion == "ghost" && slot == 1 {
        resolve(&text.replace("{Time}", "{UseCount}"), spec)
    } else {
        resolve(text, spec)
    }
}
/// The slot letter a skill title starts with ("Q   Name"; the name alone
/// when the game has none). The skills' names, not their keys: the HUD
/// strips it and shows the bound key in the tooltip's corner instead.
pub const SKILL_LETTERS: [&str; 3] = ["Q", "W", "R"];
pub fn skill(ctx: &StableClient<'_>, champion: &str, slot: usize, spec: Option<&Value>) -> String {
    let tag = ["skill", "skill2", "ult"][slot];
    let name_tag = ["skill1", "skill2", "ult"][slot];
    let key = SKILL_LETTERS[slot];
    let name = translated(
        ctx,
        &format!("#asset/base/text/champion?skill_name.{champion}.{name_tag}"),
    )
    .unwrap_or_else(|| key.into());
    let fallback = format!("#asset/base/text/champion?description.{champion}.{tag}");
    let reference = spec
        .and_then(|s| skill_definition(slot, s))
        .and_then(|s| s.get("description"))
        .and_then(Value::as_str)
        .unwrap_or(&fallback);
    let template = translated(ctx, reference);
    // A Workshop action can supply native text without a template accessible
    // through the SDK. Still request it, and retain a localized cache identity.
    let identity = template.clone().unwrap_or_else(|| {
        let locale = translated(
            ctx,
            "#asset/base/text/champion?description.illusionist.skill",
        )
        .unwrap_or_default();
        format!("{reference}\n{locale}")
    });
    let description = crate::native_tooltips::description(champion, slot, &identity)
        .map(|s| rich_with_icons(&s))
        .unwrap_or_else(|| {
            // Clone nested effect data only when fallback is actually needed.
            let parameters = spec.and_then(|s| skill_spec(champion, slot, s));
            template.map_or("—".into(), |s| {
                resolve_skill(&s, champion, slot, parameters.as_ref())
            })
        });
    let title = if name == key {
        key.into()
    } else {
        format!("{key}   {name}")
    };
    format!("{title}\n\n{description}")
}
/// The item effect text with the game's own values, as its item panel shows
/// it (`native_tooltips::item_text`), for a template with value slots. The
/// game is asked only when the SDK finds that exact text: the game's
/// item-specific text methods stop the game when the item text sheet is
/// missing, and the SDK finding the text rules that out
/// (docs/investigation-item-text.md). None while waiting or when the game's
/// text is not complete: the template is shown as before.
fn game_item_effect(ctx: &StableClient<'_>, lookup: &str, template: &str) -> Option<String> {
    if !template.contains('{') {
        return None;
    }
    if crate::native_tooltips::claim_item_prefetch() {
        queue_every_item_effect(ctx);
    }
    ctx.i18n(&format!("#asset/base/text/item?{lookup}.option"))?;
    complete_text(crate::native_tooltips::item_text(lookup, template)?)
}
/// Once per match, at the first item that needs it: every item whose
/// effect sentence has value slots (item mods can add many), so the game's
/// item list is built once for all of them. Same rules as `item`: an
/// item's own description or `.description` text wins, and the game is
/// asked only when the SDK finds the exact `.option` text.
fn queue_every_item_effect(ctx: &StableClient<'_>) {
    let Some(Value::Object(items)) = ctx
        .setting_get_json(SettingTargetV1::ItemSetting, "")
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    else {
        return;
    };
    for (name, spec) in &items {
        let lookup = spec.get("key").and_then(Value::as_str).unwrap_or(name);
        let own = spec.get("description").and_then(Value::as_str).is_some()
            || translated(ctx, &format!("#asset/base/text/item?{lookup}.description")).is_some();
        let Some(template) = translated(ctx, &format!("#asset/base/text/item?{lookup}.option"))
        else {
            continue;
        };
        if own
            || !template.contains('{')
            || ctx
                .i18n(&format!("#asset/base/text/item?{lookup}.option"))
                .is_none()
        {
            continue;
        }
        crate::native_tooltips::item_text(lookup, &template);
    }
}
/// The game's text, if it has words and no value slot left unfilled.
fn complete_text(text: String) -> Option<String> {
    (!text.trim().is_empty() && crate::native_tooltips::unresolved_parameters(&text).is_empty())
        .then_some(text)
}
pub fn item(ctx: &StableClient<'_>, key: &str, spec: Option<&Value>) -> String {
    let name = spec
        .and_then(|s| s.get("name"))
        .and_then(Value::as_str)
        .unwrap_or(key);
    let lookup = spec
        .and_then(|s| s.get("key"))
        .and_then(Value::as_str)
        .unwrap_or(key);
    let title = translated(ctx, &format!("#asset/base/text/item?{lookup}.name"))
        .unwrap_or_else(|| name.replace('_', " "));
    let mut lines = vec![title, String::new()];
    if let Some(stats) = spec.and_then(|s| s.get("stat")).and_then(Value::as_object) {
        for (key, value) in stats {
            if !value.is_number() || value.as_f64() == Some(0.) {
                continue;
            }
            if let Some(format) = translated(ctx, &format!("#asset/base/text/item?spec.{key}")) {
                lines.push(rich_with_icons(
                    &format.replace("{Value}", &value.to_string()),
                ));
            }
        }
    }
    let explicit = spec
        .and_then(|s| s.get("description"))
        .and_then(Value::as_str)
        .and_then(|r| translated(ctx, r));
    let description = explicit
        .or_else(|| translated(ctx, &format!("#asset/base/text/item?{lookup}.description")))
        .or_else(|| {
            let template = translated(ctx, &format!("#asset/base/text/item?{lookup}.option"))?;
            Some(game_item_effect(ctx, lookup, &template).unwrap_or(template))
        });
    if let Some(description) = description {
        lines.push(String::new());
        lines.push(rich_with_icons(&description));
    }
    lines.join("\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_workshop_text_does_not_require_an_sdk_localized_template() {
        let mut asset: mod_api_stable::AssetVtableV1 = unsafe { std::mem::zeroed() };
        asset.size = std::mem::size_of_val(&asset);
        let mut raw: mod_api_stable::ClientCtxV1 = unsafe { std::mem::zeroed() };
        raw.size = std::mem::size_of_val(&raw);
        raw.asset = &asset;
        let ctx = unsafe { StableClient::from_raw(&mut raw) }.unwrap();
        crate::native_tooltips::observe(42, true);
        assert_eq!(skill(&ctx, "workshop/new", 0, None), "Q\n\n—");
        let request =
            crate::native_tooltips::take(42).expect("native request despite absent SDK text");
        assert_eq!(request.champion, "workshop/new");
        crate::native_tooltips::publish(request, Some("Deals <#ff9933ff>80<> damage.".into()));
        assert_eq!(
            skill(&ctx, "workshop/new", 0, None),
            "Q\n\nDeals <#ff9933ff>80<> damage."
        );
        crate::native_tooltips::observe(42, false);
    }
    #[test]
    fn ghost_native_q_root_bonuses_and_w_recasts_resolve_without_guessing_seconds() {
        let root = base_champion("ghost").unwrap();
        let q = skill_spec("ghost", 0, root).unwrap();
        assert_eq!(q["cooltime"], 240);
        assert_eq!(q["range"], 80000);
        assert_eq!(
            resolve_skill("{Attack} / {AttackSpeed}% / {Heal}", "ghost", 0, Some(&q)),
            "5 / 1% / 200"
        );
        let w = skill_spec("ghost", 1, root).unwrap();
        assert_eq!(
            resolve_skill("{Damage} + {Coef}% · {Time} times", "ghost", 1, Some(&w)),
            "60 + 80% · 3 times"
        );
        assert_eq!(
            resolve_skill("最多可連續使用{Time}次。", "ghost", 1, Some(&w)),
            "最多可連續使用3次。"
        );
        let r = skill_spec("ghost", 2, root).unwrap();
        assert_eq!(resolve_skill("{Time}s", "ghost", 2, Some(&r)), "5s");
        let mut changed = root.clone();
        changed["add_attack"] = 9.into();
        changed["skill2"]["charge_count"] = 4.into();
        let q = skill_spec("ghost", 0, &changed).unwrap();
        let w = skill_spec("ghost", 1, &changed).unwrap();
        assert_eq!(resolve_skill("{Attack}", "ghost", 0, Some(&q)), "9");
        assert_eq!(resolve_skill("{Time}", "ghost", 1, Some(&w)), "4");
    }
    #[test]
    fn skill_values_are_computed_from_attack_or_ability_power_with_or_without_a_base() {
        let ad = "<i#asset/base/ui/banpick/champion_stat_icon:ad_0>";
        let ap = "<i#asset/base/ui/banpick/champion_stat_icon:ap_0>";
        let hp = "<i#asset/base/ui/banpick/champion_stat_icon:hp_0>";
        // English Archer Q with 117 attack damage: 60 + 80 % = 153.6.
        let q = format!("dealing <#ff9028ff>60<> + {ad}<#ff9028ff>80% Attack Damage<> as <#ff9028ff>physical damage<>.");
        assert_eq!(
            computed(&q, 117., 200.),
            format!("dealing {ad}<#ff9028ff>154<> as <#ff9028ff>physical damage<>.")
        );
        // Korean wording, ability power, a base outside a colour run.
        let ko = format!("<#a974ffff>100<>+{ap}<#a974ffff>주문력의 60%<>만큼");
        assert_eq!(
            computed(&ko, 117., 200.),
            format!("{ap}<#a974ffff>220<>만큼")
        );
        let bare = format!("100 + {ap}<#a974ffff>60% AP<>");
        assert_eq!(computed(&bare, 0., 50.), format!("{ap}<#a974ffff>130<>"));
        // French "80 %" and Turkish "%80".
        let fr = format!("<#ff9028ff>60<> + {ad}<#ff9028ff>80 % des dégâts<>");
        assert_eq!(computed(&fr, 100., 0.), format!("{ad}<#ff9028ff>140<>"));
        let tr = format!("<#ff9028ff>60<> + {ad}<#ff9028ff>%80 saldırı<>");
        assert_eq!(computed(&tr, 100., 0.), format!("{ad}<#ff9028ff>140<>"));
        // A pure ratio (seen in game 2026-10-20, Crossbowman Q in Traditional
        // Chinese, 85 attack damage): 200 % of 85.
        let zh = format!("造成{ad}<#ff9028ff>攻擊力的200%<> <#ff9028ff>物理傷害<>。額外造成{ad}<#ff9028ff>攻擊力的60%<>傷害。");
        assert_eq!(
            computed(&zh, 85., 0.),
            format!(
                "造成{ad}<#ff9028ff>170<> <#ff9028ff>物理傷害<>。額外造成{ad}<#ff9028ff>51<>傷害。"
            )
        );
        // Left as written: other stat icons, an unresolved base, two
        // percentages in one run; several formulas each on its own.
        for kept in [
            format!("<#ef5350ff>60<> + {hp}<#ef5350ff>5% max HP<>"),
            format!("<#ff9028ff>…<> + {ad}<#ff9028ff>80% AD<>"),
            format!("<#ff9028ff>60<> + {ad}<#ff9028ff>80% AD, 20%<>"),
        ] {
            assert_eq!(computed(&kept, 117., 200.), kept);
        }
        let two = format!(
            "<#ff9028ff>10<> + {ad}<#ff9028ff>50%<> then <#a974ffff>20<> + {ap}<#a974ffff>10%<>"
        );
        assert_eq!(
            computed(&two, 100., 100.),
            format!("{ad}<#ff9028ff>60<> then {ap}<#a974ffff>30<>")
        );
        // Two stats in one total (the author, 2026-10-20: the Workshop
        // champion "tale" R, Traditional Chinese): 90 + 100 % AD + 100 % AP.
        let tale = format!("造成<#ff9028ff>90<> + {ad}<#ff9028ff>100% AD<> + {ap}<#a974ffff>100%魔法攻擊<>的<#ff9028ff>物理傷害<>");
        assert_eq!(
            computed(&tale, 280., 150.),
            format!("造成{ad}{ap}<#ff9028ff>520<>的<#ff9028ff>物理傷害<>")
        );
        // A pure ratio chain; an unreadable later term ends the total there.
        let chain = format!("{ap}<#a974ffff>50%<> + {ad}<#ff9028ff>20%<> + {hp}<#ef5350ff>5%<>");
        assert_eq!(
            computed(&chain, 100., 100.),
            format!("{ap}{ad}<#a974ffff>70<> + {hp}<#ef5350ff>5%<>")
        );
    }
    #[test]
    fn coefficient_type_follows_native_inline_stat_marker_before_it_is_stripped() {
        let spec = serde_json::json!({"attack_ratio":100,"magic_ratio":60});
        assert_eq!(
            resolve(
                "<i#asset/icons:ad_0><#ff9028ff>{Coef}%<> <i#asset/icons:ap_0><#a974ffff>{Coef}%<>",
                Some(&spec)
            ),
            "<#ff9028ff>100%<> <#a974ffff>60%<>"
        );
    }
    #[test]
    fn bomber_templates_resolve_damage_ratio_and_cc_seconds_without_losing_color() {
        let spec = base_champion("bomber").unwrap();
        assert_eq!(
            resolve(
                "<#ff9028ff>{Damage} + {Coef}%<> / {Time}s",
                spec.get("skill2")
            ),
            "<#ff9028ff>80 + 100%<> / 1.5s"
        );
        assert_eq!(
            resolve("{Damage} + {Coef}% / {Slow}% {SlowTime}s", spec.get("ult")),
            "150 + 100% / 30% 2s"
        );
        assert_eq!(resolve("{Damage}/{Coef}", spec.get("skill")), "100/100");
    }
    #[test]
    fn nested_mod_effects_use_unambiguous_declared_values_and_keep_unknowns() {
        let spec = serde_json::json!({"duration":12, "effect":{"type":"Combine", "effects":[{"damage":72,"attack_ratio":100},{"slow_ratio":30,"slow_duration":90}]}});
        assert_eq!(
            resolve(
                "{Damage} + {Coef}% {Slow}% {SlowTime}s {Duration}",
                Some(&spec)
            ),
            "72 + 100% 30% 1.5s …"
        );
        let conflicting = serde_json::json!({"effects":[{"damage":35},{"damage":80}]});
        assert_eq!(resolve("{Damage} {Unknown}", Some(&conflicting)), "… …");
        // Exact declared zero is a value, not missing data.
        assert_eq!(
            resolve("{Damage}", Some(&serde_json::json!({"damage":0}))),
            "0"
        );
    }
    #[test]
    fn added_item_localization_survives_absent_settings_and_formats_registered_stats() {
        unsafe extern "C" fn localize(
            _: *const std::ffi::c_void,
            key: *const u8,
            len: usize,
            out: *mut u8,
            cap: usize,
            out_len: *mut usize,
        ) -> bool {
            let key = std::str::from_utf8(std::slice::from_raw_parts(key, len)).unwrap();
            let value = match key {
                "asset/base/text/item?added.name" => "Added Item",
                "asset/base/text/item?added.option" => "Deals <#ff9933ff>60% attack damage<>.",
                "asset/base/text/item?spec.attack" => "<#ff9933ff>{Value} Attack Damage<>",
                _ => return false,
            };
            *out_len = value.len();
            if cap >= value.len() && !out.is_null() {
                std::ptr::copy_nonoverlapping(value.as_ptr(), out, value.len());
            }
            true
        }
        let mut asset: mod_api_stable::AssetVtableV1 = unsafe { std::mem::zeroed() };
        asset.size = std::mem::size_of_val(&asset);
        asset.i18n_get = Some(localize);
        let mut raw: mod_api_stable::ClientCtxV1 = unsafe { std::mem::zeroed() };
        raw.size = std::mem::size_of_val(&raw);
        raw.asset = &asset;
        let ctx = unsafe { StableClient::from_raw(&mut raw) }.unwrap();
        let unknown = item(&ctx, "added", None);
        assert!(unknown.starts_with("Added Item"));
        assert!(unknown.contains("<#ff9933ff>60% attack damage<>"));
        let spec = serde_json::json!({"key":"added","stat":{"attack":50,"hp":0}});
        let known = item(&ctx, "added", Some(&spec));
        assert!(known.contains("<#ff9933ff>50 Attack Damage<>"));
        assert!(!known.contains("{Value}"));
    }
    #[test]
    fn strip_rich_markup_without_inventing_formula_values() {
        assert_eq!(
            plain("Restores <#00ffffff>{Heal}<> + <i#asset/x:icon>{Coef}% health."),
            "Restores … + …% health."
        );
        assert_eq!(plain("免疫控制\n回復生命"), "免疫控制\n回復生命");
        assert!(base_champion("lancer").is_some());
    }
    #[test]
    fn retain_stat_color_runs_and_multilingual_text_without_guessing_parameters() {
        let original =
            "造成 <#ff9933ff>100 + {Ratio}% 攻擊力<> 與 <#aa55ffff>60% 魔法傷害<>。<i#asset/icon>";
        let colored = rich(original);
        assert_eq!(
            colored,
            "造成 <#ff9933ff>100 + …% 攻擊力<> 與 <#aa55ffff>60% 魔法傷害<>。"
        );
        assert_eq!(plain(&colored), "造成 100 + …% 攻擊力 與 60% 魔法傷害。");
        assert_eq!(rich("<#invalid>text<>"), "text<>");
    }
    #[test]
    fn item_text_keeps_only_well_formed_inline_stat_icons() {
        let riot = "<i#asset/base/ui/banpick/champion_stat_icon:hp_0> <#60e84dff>maximum health<>";
        assert_eq!(rich_with_icons(riot), riot);
        assert_eq!(rich(riot), " <#60e84dff>maximum health<>");
        assert_eq!(rich_with_icons("<i#asset/icon>x<i#other:a>"), "x");
    }
    #[test]
    fn the_game_item_text_is_used_only_when_complete() {
        // Read from the game (Beta 2, giants_horn_shard): its text method
        // fills every slot; anything else keeps the template as before.
        let filled =
            "每秒回復 40 生命值，並對周圍 30 範圍內所有敵人造成 10+最大生命值1% 的魔法傷害。";
        assert_eq!(complete_text(filled.into()), Some(filled.into()));
        assert_eq!(complete_text("Recovers {Flat} HP".into()), None);
        assert_eq!(complete_text("  ".into()), None);
        // Braces that are not a value slot are kept.
        assert_eq!(
            complete_text("Deals 10 {".into()),
            Some("Deals 10 {".into())
        );
    }
}
