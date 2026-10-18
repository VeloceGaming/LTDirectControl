//! Hook families, and the layout guards a game version's tables carry
//! (native_adapter/v*/identity.rs); never runtime-scanned.
/// Hook families. Each is verified and installed on its own, so a game
/// build can be supported one family at a time: a family is listed in
/// `ENABLED` only when its anchors, its layout guards below and every raw
/// field offset its hooks use have been reviewed for this executable.
pub mod family {
    /// Worker, viewer and input hooks: frame pacing and the session.
    pub const SESSION: u32 = 1;
    /// Camera, vision and the full-screen layout lease inside the viewer.
    pub const CAMERA: u32 = 1 << 1;
    /// Move, stop, recall-cancel and steering.
    pub const MOVEMENT: u32 = 1 << 2;
    /// Attack, auto-attack, skills, aim and the entity fields they read.
    pub const COMBAT: u32 = 1 << 3;
    /// Outline shader and unit renderer.
    pub const OUTLINE: u32 = 1 << 4;
    pub const MINIMAP: u32 = 1 << 5;
    /// Gold and build readers, and the purchase hooks.
    pub const SHOP: u32 = 1 << 6;
    /// Skill geometry and effect-tree readers (skill previews).
    pub const READERS: u32 = 1 << 7;
    /// Native ability description text.
    pub const TOOLTIPS: u32 = 1 << 8;
    pub const ALL: u32 = (1 << 9) - 1;
    /// Athlete stat rules for manually played matches (separate from `ALL`:
    /// a family is enabled by name, and the guards fall back to `ALL`).
    pub const STATS: u32 = 1 << 9;
    /// Families another mod may already hold: their hooks only draw, so
    /// the others install without them (2026-10-18: a champion pack's
    /// high-definition frames hook the unit renderer on game 0.6.4).
    pub const MAY_STEP_ASIDE: u32 = OUTLINE;
}
/// The families to install from `enabled`, given each checked spot as
/// (family, rva, holds the expected bytes). A differing spot of a family
/// that may step aside drops that family only; the spots it lost are
/// returned for the log. A differing spot of any other family refuses
/// everything: `Err` names the first one.
pub fn families_to_install(
    enabled: u32,
    spots: &[(u32, usize, bool)],
) -> Result<(u32, Vec<usize>), usize> {
    let mut families = enabled;
    let mut taken = Vec::new();
    for &(owner, rva, holds) in spots {
        if enabled & owner == 0 || holds {
            continue;
        }
        if family::MAY_STEP_ASIDE & owner == owner {
            families &= !owner;
            taken.push(rva);
        } else {
            return Err(rva);
        }
    }
    Ok((families, taken))
}
/// A game version's field-consumer guards: instruction bytes and pointer
/// slots that must hold before its hooks are installed.
pub struct Guards {
    pub bytes: &'static [(usize, &'static [u8])],
    pub pointers: &'static [(usize, usize)],
    /// The family each guard protects, by guard address.
    pub families: &'static [(u32, &'static [usize])],
}
impl Guards {
    fn family(&self, rva: usize) -> u32 {
        self.families
            .iter()
            .find(|(_, guards)| guards.contains(&rva))
            .map_or(family::ALL, |(family, _)| *family)
    }
    /// Call only after the exact disk hash and loaded PE identity have
    /// passed. Checks the guards of the `families` being installed; a guard
    /// without a listed family is always checked.
    pub unsafe fn verify(&self, base: usize, families: u32) -> bool {
        let wanted = |family: u32| family == family::ALL || families & family != 0;
        self.bytes
            .iter()
            .filter(|(rva, _)| wanted(self.family(*rva)))
            .all(|(rva, bytes)| {
                std::slice::from_raw_parts((base + rva) as *const u8, bytes.len()) == *bytes
            })
            && (families & family::READERS == 0
                || self.pointers.iter().all(|(rva, target)| {
                    std::ptr::read_unaligned((base + rva) as *const usize) == base + target
                }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_family_that_may_step_aside_is_dropped_when_another_mod_holds_it() {
        let enabled = family::ALL;
        let ok = [(family::SESSION, 0x10, true), (family::OUTLINE, 0x20, true)];
        assert_eq!(families_to_install(enabled, &ok), Ok((enabled, vec![])));
        // The unit renderer taken: everything but the outline installs.
        let taken = [
            (family::SESSION, 0x10, true),
            (family::OUTLINE, 0x20, false),
            (family::OUTLINE, 0x30, false),
            (family::COMBAT, 0x40, true),
        ];
        assert_eq!(
            families_to_install(enabled, &taken),
            Ok((enabled & !family::OUTLINE, vec![0x20, 0x30]))
        );
        // Any other family differing still refuses everything.
        let moved = [
            (family::OUTLINE, 0x20, false),
            (family::MOVEMENT, 0x50, false),
        ];
        assert_eq!(families_to_install(enabled, &moved), Err(0x50));
        // A family that is not enabled is not checked.
        assert_eq!(
            families_to_install(family::SESSION, &moved),
            Ok((family::SESSION, vec![]))
        );
    }
}
