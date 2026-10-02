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
        .and_then(|s| s.get(tag))
        .and_then(|s| s.get("description"))
        .and_then(Value::as_str)
        .unwrap_or(&fallback);
    let description = translated(ctx, reference).map_or("—".into(), |s| plain(&s));
    let title = if name == key {
        key.into()
    } else {
        format!("{key}   {name}")
    };
    format!("{title}\n\n{description}")
}
pub fn item(ctx: &StableClient<'_>, key: &str, spec: Option<&Value>) -> String {
    let Some(spec) = spec else {
        return "—".into();
    };
    let name = spec.get("name").and_then(Value::as_str).unwrap_or(key);
    let lookup = spec.get("key").and_then(Value::as_str).unwrap_or(key);
    let title = translated(ctx, &format!("#asset/base/text/item?{lookup}.name"))
        .unwrap_or_else(|| name.replace('_', " "));
    let mut lines = vec![title, String::new()];
    if let Some(stats) = spec.get("stat").and_then(Value::as_object) {
        for (key, value) in stats {
            if !value.is_number() || value.as_f64() == Some(0.) {
                continue;
            }
            if let Some(format) = translated(ctx, &format!("#asset/base/text/item?spec.{key}")) {
                lines.push(plain(&format.replace("{Value}", &value.to_string())));
            }
        }
    }
    let explicit = spec
        .get("description")
        .and_then(Value::as_str)
        .and_then(|r| translated(ctx, r));
    let description = explicit
        .or_else(|| translated(ctx, &format!("#asset/base/text/item?{lookup}.description")))
        .or_else(|| translated(ctx, &format!("#asset/base/text/item?{lookup}.option")));
    if let Some(description) = description {
        lines.push(String::new());
        lines.push(plain(&description));
    }
    lines.join("\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_rich_markup_without_inventing_formula_values() {
        assert_eq!(
            plain("Restores <#00ffffff>{Heal}<> + <i#asset/x:icon>{Coef}% health."),
            "Restores … + …% health."
        );
        assert_eq!(plain("免疫控制\n回復生命"), "免疫控制\n回復生命");
        assert!(base_champion("lancer").is_some());
    }
}
