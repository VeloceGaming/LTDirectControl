//! Host-localized descriptions. Unknown formula parameters remain visibly unknown.
use mod_api_stable::StableClient;
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
pub fn skill(ctx: &StableClient<'_>, champion: &str, slot: usize, spec: Option<&Value>) -> String {
    let tag = ["skill", "skill2", "ult"][slot];
    let name_tag = ["skill1", "skill2", "ult"][slot];
    let key = ["Q", "W", "R"][slot];
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
        .or_else(|| translated(ctx, &format!("#asset/base/text/item?{lookup}.option")));
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
}
