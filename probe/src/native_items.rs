//! Owned copies of effective registered items, on the fingerprinted 0.6.3 host.
//! Native objects are inspected only inside their original AI callback.
use mod_api_stable::StableAiContext;
use serde_json::Value;
use std::collections::HashMap;

pub type Catalogue = HashMap<String, Value>;
#[cfg(all(windows, target_arch = "x86_64"))]
const ANCHORS: &[(usize, &[u8])] = &[
    // Native buying/AI consumer: context +8 is the settings owner.
    // The old +1b8 chain accidentally read the simulation tick.
    (0xdeb303, &[0x48, 0x8b, 0x42, 0x08]),
    (0xea1687, &[0x49, 0x8b, 0x59, 0x08]),
    (
        0x2e83dd0,
        &[
            0x48, 0x85, 0xc9, 0x74, 0x0c, 0x48, 0x8b, 0x41, 0x08, 0x48, 0x8b, 0x80, 0xf8, 0x07,
            0x00, 0x00, 0xc3,
        ],
    ),
    (
        0x2e84805,
        &[
            0x48, 0x8b, 0x41, 0x10, 0x48, 0x8b, 0x00, 0x48, 0x8b, 0x80, 0xb8, 0x01, 0x00, 0x00,
            0xc3,
        ],
    ),
    (
        0xea168d,
        &[
            0x48, 0x8b, 0x43, 0x30, 0x48, 0x8b, 0x58, 0x08, 0x4c, 0x8b, 0x70, 0x10,
        ],
    ),
    (
        0x27e9726,
        &[
            0x49, 0x8b, 0x7f, 0x78, 0x49, 0x8b, 0xb7, 0x90, 0x00, 0x00, 0x00,
        ],
    ),
    (0xea18ad, &[0xff, 0x90, 0x80, 0x00, 0x00, 0x00]),
    (0x1322f80, &[0x48, 0x8d, 0x41, 0x30, 0xc3]),
    (0x2e67084, &[0xff, 0x53, 0x50]),
    (0x2e670ea, &[0x0f, 0x11, 0x46, 0x30]),
];

// Getter +0x80 borrows NEXT tiers from wrapper +0x30. Native buying
// searches these forward edges backward from a goal (ea1730). Wrapper
// constructor 2e66f50 caches legacy +0x50 there; +0x58 goes to +0x48.
// Keep the forward direction and derive previous links for diagnostics only.
fn link_upgrades(items: &mut Catalogue) -> Option<()> {
    for item in items.values_mut() {
        let next = item["next_tier"].as_array_mut()?;
        next.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        next.dedup();
        item["previous_tier"] = serde_json::json!([]);
    }
    let edges = items
        .iter()
        .flat_map(|(key, item)| {
            item["next_tier"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|next| (key.clone(), next.to_owned()))
        })
        .collect::<Vec<_>>();
    for (previous, next) in edges {
        // Keep native forward edges verbatim. An absent optional destination
        // must not discard every item's effective price/stats. Forecast paths
        // still require each traversed item to exist.
        if let Some(item) = items.get_mut(&next) {
            item["previous_tier"].as_array_mut()?.push(previous.into());
        }
    }
    Some(())
}

pub fn read(ctx: &StableAiContext<'_>) -> Result<Catalogue, String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        let base =
            crate::native_adapter::verified_base().ok_or("native adapter is not installed")?;
        for (rva, bytes) in ANCHORS {
            if std::slice::from_raw_parts((base + rva) as *const u8, bytes.len()) != *bytes {
                return Err(format!(
                    "registered item instruction anchor changed at {rva:x}"
                ));
            }
        }
        ctx.with_native_context(|state, table| windows::read(base, state as usize, table as usize))
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let _ = ctx;
        Err("registered item reader requires Windows x64".into())
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod windows {
    use super::*;
    unsafe fn word(at: usize) -> usize {
        std::ptr::read_unaligned(at as *const usize)
    }
    unsafe fn vector(at: usize, limit: usize) -> Option<(usize, usize)> {
        if at < 0x10000 || !at.is_multiple_of(8) {
            return None;
        }
        let (cap, ptr, len) = (word(at), word(at + 8), word(at + 16));
        if len > limit || cap < len || cap > limit * 16 || (len > 0 && ptr < 0x10000) {
            return None;
        }
        Some((ptr, len))
    }
    // Rust String/Vec layout here comes from native getter callers. It is not
    // assumed to match this DLL's compiler layout; inspect three raw words.
    unsafe fn string(at: usize) -> Option<String> {
        let (ptr, len) = vector(at, 4096)?;
        if len == 0 {
            return Some(String::new());
        }
        std::str::from_utf8(std::slice::from_raw_parts(ptr as *const u8, len))
            .ok()
            .map(str::to_owned)
    }
    unsafe fn strings(at: usize) -> Option<Vec<String>> {
        let (ptr, len) = vector(at, 4096)?;
        if len > 0 && !ptr.is_multiple_of(8) {
            return None;
        }
        (0..len).map(|i| string(ptr + i * 24)).collect()
    }
    unsafe fn method(table: usize, offset: usize) -> Option<usize> {
        let address = word(table + offset);
        (address >= 0x10000).then_some(address)
    }
    unsafe fn borrowed_string(object: usize, table: usize, offset: usize) -> Option<String> {
        let get: unsafe extern "system" fn(usize) -> usize =
            std::mem::transmute(method(table, offset)?);
        string(get(object))
    }
    unsafe fn scalar(object: usize, table: usize, offset: usize) -> Option<usize> {
        let get: unsafe extern "system" fn(usize) -> usize =
            std::mem::transmute(method(table, offset)?);
        Some(get(object))
    }
    unsafe fn stats(object: usize, table: usize) -> Option<Value> {
        // Native tooltip 27e9090 uses the same sret POD, without drop glue.
        // Its named field loads establish offsets; this is NOT SDK BuffV1.
        let get: unsafe extern "system" fn(*mut usize, usize) -> *mut usize =
            std::mem::transmute(method(table, 0x78)?);
        let mut buff = [0usize; 64];
        get(buff.as_mut_ptr(), object);
        let ptr = buff.as_ptr() as usize;
        let mut result = serde_json::Map::new();
        for (offset, name) in [
            (0x58, "attack"),
            (0x60, "magic_power"),
            (0x68, "defence"),
            (0x70, "hp"),
            (0x74, "hp_regen"),
            (0x78, "magic_resistance"),
            (0x80, "vamp"),
            (0x84, "hp_mult"),
            (0x88, "move_speed_mult"),
            (0x8c, "attack_speed_mult"),
            (0x90, "skill_cooldown_mult"),
            (0x104, "crit_chance"),
        ] {
            result.insert(
                name.into(),
                std::ptr::read_unaligned((ptr + offset) as *const i32).into(),
            );
        }
        for (offset, name) in [
            (0x98, "reflect"),
            (0xa8, "defence_penetration"),
            (0xb0, "magic_resistance_penetration"),
            (0xb8, "toughness"),
            (0xc0, "heal_reduce"),
            (0xc8, "range"),
            (0xd0, "attack_enemy_hp_ratio"),
            (0xd8, "attack_my_hp_ratio"),
            (0xe0, "skill_enemy_hp_ratio"),
            (0xf0, "dot_amplify"),
            (0x108, "base_attack_damaged_reduce"),
            (0x110, "skill_damaged_reduce"),
        ] {
            let value = word(ptr + offset);
            result.insert(
                name.into(),
                (if name == "range" { value / 1000 } else { value }).into(),
            );
        }
        Some(Value::Object(result))
    }
    unsafe fn registry(context: usize) -> Result<(usize, usize), String> {
        let settings = word(context + 8);
        if settings < 0x10000 {
            return Err("native buying settings pointer rejected".into());
        }
        let (ptr, len) =
            vector(word(settings + 0x30), 4096).ok_or("registered vector bounds rejected")?;
        if len == 0 || !ptr.is_multiple_of(8) {
            return Err(format!("registered vector empty or unaligned: count={len}"));
        }
        Ok((ptr, len))
    }
    pub(super) unsafe fn read(
        base: usize,
        state: usize,
        table: usize,
    ) -> Result<Catalogue, String> {
        // Full executable hash and instruction anchors are checked at install.
        // Require the observed native AI bridge, before interpreting its state.
        if state < 0x10000
            || table != base + 0x3d10330
            || word(table) < 120
            || word(table + 8) != base + 0x2e83dd0
            || word(table + 48) != base + 0x2e84800
        {
            return Err("AI bridge identity rejected".into());
        }
        let context = word(state + 0x10);
        if context < 0x10000 {
            return Err("AI context pointer rejected".into());
        }
        let (ptr, len) = registry(context)?;
        let mut items = Catalogue::new();
        for i in 0..len {
            let object = word(ptr + i * 16);
            let table = word(ptr + i * 16 + 8);
            if object < 0x10000 || table < 0x10000 {
                return Err(format!("registered item {i}/{len}: object/table rejected"));
            }
            let enabled: unsafe extern "system" fn(usize) -> bool = std::mem::transmute(
                method(table, 0x50).ok_or_else(|| format!("item {i}: enabled getter rejected"))?,
            );
            let key = borrowed_string(object, table, 0x58)
                .ok_or_else(|| format!("item {i}: key getter/string rejected"))?;
            if key.is_empty() {
                return Err(format!("item {i}: empty key"));
            }
            let icon = borrowed_string(object, table, 0x60)
                .ok_or_else(|| format!("item {i} {key}: icon string rejected"))?;
            let price = scalar(object, table, 0x68)
                .ok_or_else(|| format!("item {i} {key}: price getter rejected"))?;
            let tier = scalar(object, table, 0x70)
                .ok_or_else(|| format!("item {i} {key}: tier getter rejected"))?;
            let next: unsafe extern "system" fn(usize) -> usize = std::mem::transmute(
                method(table, 0x80)
                    .ok_or_else(|| format!("item {i} {key}: upgrade getter rejected"))?,
            );
            let next = strings(next(object))
                .ok_or_else(|| format!("item {i} {key}: upgrade strings rejected"))?;
            let stat = stats(object, table)
                .ok_or_else(|| format!("item {i} {key}: stat getter rejected"))?;
            let spec = serde_json::json!({"key":key,"name":key,"icon":icon,"price":price,"tier":tier,
                "enabled":enabled(object),"stat":stat,"next_tier":next});
            if items.insert(key, spec).is_some() {
                return Err(format!("item {i}: duplicate key"));
            }
        }
        link_upgrades(&mut items).ok_or("upgrade metadata malformed")?;
        Ok(items)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn registry_uses_settings_owner_not_simulation_tick() {
            let mut entries = [1usize, 2];
            let vector = [1usize, entries.as_mut_ptr() as usize, 1];
            let mut settings = [0usize; 7];
            settings[6] = vector.as_ptr() as usize;
            let simulation = [0usize; 60];
            let context = [simulation.as_ptr() as usize, settings.as_ptr() as usize];
            assert_eq!(
                unsafe { registry(context.as_ptr() as usize) }.unwrap(),
                (entries.as_ptr() as usize, 1)
            );
            let bad_context = [simulation.as_ptr() as usize, 0];
            assert!(unsafe { registry(bad_context.as_ptr() as usize) }
                .unwrap_err()
                .contains("settings pointer"));
        }
        #[test]
        fn borrowed_native_strings_are_owned_and_stat_getter_uses_sret_abi() {
            let mut data = b"mod_item".to_vec();
            let borrowed = [data.capacity(), data.as_mut_ptr() as usize, data.len()];
            let copy = unsafe { string(borrowed.as_ptr() as usize) }.unwrap();
            drop(data);
            assert_eq!(copy, "mod_item");
            unsafe extern "system" fn getter(out: *mut usize, object: usize) -> *mut usize {
                assert_eq!(object, 7);
                std::ptr::write_unaligned((out as usize + 0x58) as *mut i32, 50);
                std::ptr::write_unaligned((out as usize + 0xc8) as *mut usize, 12_000);
                out
            }
            let mut vtable = [0usize; 17];
            vtable[0x78 / 8] = getter as *const () as usize;
            let copied = unsafe { stats(7, vtable.as_ptr() as usize) }.unwrap();
            assert_eq!(copied["attack"], 50);
            assert_eq!(copied["range"], 12);
            assert_eq!(copied["hp"], 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forward_catalogue_forecasts_owned_component_upgrade_and_affordability() {
        // Synthetic prices exercise graph direction, not item-mod balance data.
        let mut items = Catalogue::from([
            (
                "ironsword".into(),
                serde_json::json!({"price":250,"tier":0,"next_tier":["ruinous_blade"]}),
            ),
            (
                "ruinous_blade".into(),
                serde_json::json!({"price":400,"tier":1,"next_tier":["endless_hunger"]}),
            ),
            (
                "endless_hunger".into(),
                serde_json::json!({"price":600,"tier":2,"next_tier":["radiant_endless_hunger"]}),
            ),
            (
                "radiant_endless_hunger".into(),
                serde_json::json!({"price":800,"tier":3,"next_tier":[]}),
            ),
        ]);
        link_upgrades(&mut items).unwrap();
        let metadata = crate::purchase_tracker::metadata(&items);
        let plan = ["radiant_endless_hunger".into()];
        let owned = ["ironsword".into()];
        let f = crate::purchase_tracker::forecast(&metadata, &plan, &owned, 399).unwrap();
        assert_eq!(f.next.unwrap().key, "ruinous_blade");
        assert!(f.affordable.is_empty());
        let f = crate::purchase_tracker::forecast(&metadata, &plan, &owned, 1000).unwrap();
        assert_eq!(
            f.affordable
                .iter()
                .map(|p| p.key.as_str())
                .collect::<Vec<_>>(),
            ["ruinous_blade", "endless_hunger"]
        );
    }
    #[test]
    fn registered_forward_edges_stay_forward_and_recover_predecessors() {
        let mut items = Catalogue::from([
            (
                "base".into(),
                serde_json::json!({"next_tier":["added","other","added"]}),
            ),
            ("added".into(), serde_json::json!({"next_tier":[]})),
            ("other".into(), serde_json::json!({"next_tier":[]})),
        ]);
        assert_eq!(link_upgrades(&mut items), Some(()));
        assert_eq!(
            items["base"]["next_tier"],
            serde_json::json!(["added", "other"])
        );
        assert_eq!(items["added"]["next_tier"], serde_json::json!([]));
        assert_eq!(items["added"]["previous_tier"], serde_json::json!(["base"]));
        items.get_mut("added").unwrap()["next_tier"] = serde_json::json!(["unknown"]);
        assert_eq!(link_upgrades(&mut items), Some(()));
        assert_eq!(items["added"]["next_tier"], serde_json::json!(["unknown"]));
        assert!(items.contains_key("base"));
    }
}
