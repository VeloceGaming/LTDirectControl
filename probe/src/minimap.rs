//! Minimap appearance and geometry; no simulation or item-build changes.
use crate::camera::{CameraControl, Rect};
use mod_api_stable::StableClient;

pub const MAP_SIZE: f32 = 352.;
pub const PADDING: f32 = 4.;
pub const STRIP_END: usize = 1560;

pub fn content_rect(wide: bool, left: bool, custom: Option<(f32, f32)>) -> Rect {
    let (x, y) = custom.unwrap_or(if wide {
        (1560., 720.)
    } else if left {
        (995., 711.)
    } else {
        (565., 711.)
    });
    Rect {
        x: x + PADDING,
        y: y + PADDING,
        w: MAP_SIZE,
        h: MAP_SIZE,
    }
}

pub fn draw_frame(ctx: &mut StableClient<'_>, camera: &CameraControl) {
    let Some(frame) = camera.frame() else { return };
    if frame.minimap.w != MAP_SIZE || frame.minimap.h != MAP_SIZE {
        return;
    }
    let r = frame.minimap;
    let x = r.x - PADDING;
    let y = r.y - PADDING;
    let size = MAP_SIZE + 2. * PADDING;
    // The native renderer supplies the flat base and rounded outer corners.
    // These strokes stay entirely in the padding, clear of markers and clicks.
    for (a, b) in [
        ((x + 3., y + 0.5), (x + size - 3., y + 0.5)),
        ((x + 3., y + size - 0.5), (x + size - 3., y + size - 0.5)),
        ((x + 0.5, y + 3.), (x + 0.5, y + size - 3.)),
        ((x + size - 0.5, y + 3.), (x + size - 0.5, y + size - 3.)),
    ] {
        ctx.draw_line(
            "UI",
            a.0,
            a.1,
            b.0,
            b.1,
            1.,
            1002,
            crate::ui_theme::tone(0x4b4a49ff),
        );
    }
    for (cx, cy, sx, sy) in [
        (x, y, 1., 1.),
        (x + size, y, -1., 1.),
        (x, y + size, 1., -1.),
        (x + size, y + size, -1., -1.),
    ] {
        let point = |a: f32, b: f32| (cx + sx * a, cy + sy * b);
        for (a, b) in [
            ((0.75, 11.5), (0.75, 4.5)),
            ((0.75, 4.5), (4.5, 0.75)),
            ((4.5, 0.75), (11.5, 0.75)),
        ] {
            let a = point(a.0, a.1);
            let b = point(b.0, b.1);
            ctx.draw_line("UI", a.0, a.1, b.0, b.1, 1.5, 1003, 0x817c77ff);
        }
        let a = point(2.25, 5.75);
        let b = point(5.75, 2.25);
        ctx.draw_line("UI", a.0, a.1, b.0, b.1, 1., 1003, 0x817c7773);
    }
}

#[cfg(any(test, all(windows, target_arch = "x86_64")))]
pub(crate) mod native {
    use serde_json::Value;
    const PROFILE: &str = include_str!("../../tools/native_profiles/0.6.3.json");
    pub struct BytePatch {
        pub site: usize,
        pub expected: Vec<u8>,
        pub bytes: Vec<u8>,
    }
    pub struct Plan {
        pub patches: Vec<BytePatch>,
        pub constants: Vec<u8>,
        pub guards: Vec<(usize, Vec<u8>)>,
    }
    fn hex(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) {
            return Err("Odd minimap guard length".into());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "Invalid minimap guard".into())
            })
            .collect()
    }
    fn rva(v: &Value) -> Result<usize, String> {
        usize::from_str_radix(
            v.as_str()
                .ok_or("Missing minimap RVA")?
                .trim_start_matches("0x"),
            16,
        )
        .map_err(|_| "Invalid minimap RVA".into())
    }
    pub fn plan(base: usize, constants_address: usize) -> Result<Plan, String> {
        let p: Value = serde_json::from_str(PROFILE).map_err(|e| e.to_string())?;
        let map = &p["minimap"];
        if map["reviewed"] != true || map["map_size"] != 352 || map["padding"] != 4 {
            return Err("Unreviewed minimap geometry".into());
        }
        let mut plan = Plan {
            patches: Vec::new(),
            constants: Vec::new(),
            guards: Vec::new(),
        };
        for op in map["operands"]
            .as_array()
            .ok_or("Missing minimap operands")?
        {
            let site = base
                .checked_add(rva(&op["rva"])?)
                .ok_or("Minimap address overflow")?;
            let expected = hex(op["bytes"].as_str().ok_or("Missing minimap bytes")?)?;
            let offset = op["offset"]
                .as_u64()
                .ok_or("Missing minimap operand offset")? as usize;
            let mut bytes = expected.clone();
            let replacement = match op["kind"].as_str() {
                Some("rip_f32x4") => {
                    let values = op["values"]
                        .as_array()
                        .filter(|v| v.len() == 4)
                        .ok_or("Invalid minimap constant")?;
                    let address = constants_address
                        .checked_add(plan.constants.len())
                        .ok_or("Minimap constant overflow")?;
                    let delta =
                        i32::try_from(address as i128 - (site as i128 + bytes.len() as i128))
                            .map_err(|_| "Minimap constants outside RIP reach")?;
                    for v in values {
                        let v = v.as_f64().ok_or("Invalid minimap float")? as f32;
                        if !v.is_finite() {
                            return Err("Nonfinite minimap float".into());
                        }
                        plan.constants.extend_from_slice(&v.to_le_bytes());
                    }
                    let guard_address = base
                        .checked_add(rva(&op["data_rva"])?)
                        .ok_or("Minimap guard overflow")?;
                    plan.guards.push((
                        guard_address,
                        hex(op["data_bytes"]
                            .as_str()
                            .ok_or("Missing minimap data guard")?)?,
                    ));
                    delta.to_le_bytes().to_vec()
                }
                Some("immediate") => {
                    hex(op["value"].as_str().ok_or("Missing minimap immediate")?)?
                }
                _ => return Err("Unknown minimap operand".into()),
            };
            bytes
                .get_mut(offset..offset + replacement.len())
                .ok_or("Minimap operand outside instruction")?
                .copy_from_slice(&replacement);
            plan.patches.push(BytePatch {
                site,
                expected,
                bytes,
            });
        }
        if plan.constants.len() > 0x1000 || !constants_address.is_multiple_of(16) {
            return Err("Minimap constant block alignment/size differs".into());
        }
        Ok(plan)
    }
    #[cfg(all(windows, target_arch = "x86_64"))]
    impl Plan {
        /// These addresses all belong to the fingerprinted resident executable.
        pub unsafe fn verify(&self) -> bool {
            self.patches.iter().all(|p| {
                std::slice::from_raw_parts(p.site as *const u8, p.expected.len()) == p.expected
            }) && self
                .guards
                .iter()
                .all(|(site, b)| std::slice::from_raw_parts(*site as *const u8, b.len()) == b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraFrame;
    #[test]
    fn map_projection_matches_enlarged_native_camera_in_every_layout() {
        for (wide, left, custom) in [
            (true, false, None),
            (false, false, None),
            (false, true, None),
            (true, false, Some((200., 300.))),
        ] {
            let r = content_rect(wide, left, custom);
            let f = CameraFrame {
                viewport: Rect {
                    x: 0.,
                    y: 50.,
                    w: 1920.,
                    h: 974.,
                },
                center: (480., 480.),
                extent: (1920., 1920.),
                minimap: r,
            };
            for p in [
                (0, 0),
                (480_000, 480_000),
                (123_000, 789_000),
                (959_000, 959_000),
            ] {
                let screen = f.project_minimap(p);
                assert_eq!(f.unproject_minimap(screen), Some(p));
                let native = (
                    (screen.0 - r.x) * (960. / MAP_SIZE),
                    (screen.1 - r.y) * (960. / MAP_SIZE),
                );
                assert!((native.0 - p.0 as f32 / 1000.).abs() < 0.001);
                assert!((native.1 - p.1 as f32 / 1000.).abs() < 0.001);
            }
            assert!(f.unproject_minimap((r.x - 1., r.y)).is_none());
            assert!(f.unproject_minimap((r.x + MAP_SIZE, r.y)).is_none());
        }
        assert_eq!(
            content_rect(true, false, None),
            Rect {
                x: 1564.,
                y: 724.,
                w: 352.,
                h: 352.
            }
        );
    }
    #[test]
    fn isolated_constants_only_replace_reviewed_operands_and_remain_in_reach() {
        let base = 0x140000000usize;
        let constants = base + 0x6000000;
        let p = native::plan(base, constants).unwrap();
        assert_eq!(p.patches.len(), 54);
        assert!(!p.guards.is_empty());
        assert_eq!(p.constants.len() % 16, 0);
        assert!(p.constants.len() < 4096);
        for patch in p.patches {
            assert_eq!(patch.bytes.len(), patch.expected.len());
            assert_ne!(patch.bytes, patch.expected);
            assert!((base..base + 0x52d6000).contains(&patch.site));
        }
        assert!(native::plan(base, usize::MAX & !15).is_err());
        assert!(native::plan(base, constants + 1).is_err());
    }
}
