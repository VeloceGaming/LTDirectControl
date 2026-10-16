//! Game 0.6.3: where the athlete stat rules act (see `crate::athlete_rules` and
//! docs/investigation-stat-rules-0.6.4.md). Every site replaces whole
//! instructions that are position independent; the hook replays them and then
//! lets the rules adjust one register or one frame slot. The verifier
//! (tools/verify_native_profile.py) decodes each site and checks the registers
//! and offsets written here against the executable.
use crate::athlete_rules::{Caster, Kind, Register as R, Site, Source};

/// Offset of the caster's actor id inside a projectile object.
pub(crate) const STAT_PROJECTILE_CASTER: usize = 0xf8;
/// Offset of the actor id inside a champion entity.
pub(crate) const STAT_ENTITY_ID: usize = 0x5b8;
/// Offset of the per-player seed the reaction gates mix into their hash.
pub(crate) const STAT_PLAYER_SEED: usize = 0x7f8;
/// Offset of the derived Dodging term inside an entity (Dodging x 150).
pub(crate) const STAT_DERIVED_DODGING: usize = 0x6a0;
/// Offset of a projectile's kind, the word the hit code switches on after
/// the projectile Accuracy producer (traced by `STATRULE trace`).
pub(crate) const STAT_TRACE_PROJECTILE_KIND: usize = 0x40;
/// Offset of an entity's action state, the word the code switches on before
/// the entity Accuracy producers (traced by `STATRULE trace`).
pub(crate) const STAT_TRACE_ACTION_STATE: usize = 0x300;

pub(crate) const STAT_SITES: &[Site] = &[
    // projectile gate: capture the inputs, then replace the gate.
    Site {
        rva: 0xe25856,
        bytes: &[0x49, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Capture {
            accuracy: R::Rcx,
            inverse_dodge: Source::Register(R::R8),
            caster: Caster::Field {
                reg: R::Rdi,
                offset: STAT_PROJECTILE_CASTER,
            },
            age_mixed: false,
        },
    },
    Site {
        rva: 0xe258d2,
        bytes: &[0xc1, 0xe9, 0x11, 0x48, 0x01, 0xc1],
        kind: Kind::Complete {
            capture: 0,
            output: R::Rcx,
        },
    },
    // big AI function, path 1: capture the inputs, then replace the gate.
    Site {
        rva: 0xe167ce,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Capture {
            accuracy: R::Rcx,
            inverse_dodge: Source::Frame(0x5b0),
            caster: Caster::Field {
                reg: R::R14,
                offset: STAT_PROJECTILE_CASTER,
            },
            age_mixed: false,
        },
    },
    Site {
        rva: 0xe16857,
        bytes: &[0xc1, 0xe9, 0x11, 0x48, 0x01, 0xc1],
        kind: Kind::Complete {
            capture: 2,
            output: R::Rcx,
        },
    },
    // big AI function, path 2: capture the inputs, then replace the gate.
    Site {
        rva: 0xe16b3f,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Capture {
            accuracy: R::R8,
            inverse_dodge: Source::Frame(0x5b0),
            caster: Caster::Shifted {
                reg: R::R13,
                shift: 20,
            },
            age_mixed: true,
        },
    },
    Site {
        rva: 0xe16bcc,
        bytes: &[0xc1, 0xea, 0x11, 0x48, 0x01, 0xc2],
        kind: Kind::Complete {
            capture: 4,
            output: R::Rdx,
        },
    },
    // area/casted-effect gate: capture the inputs, then replace the gate.
    Site {
        rva: 0xed2cf1,
        bytes: &[0x49, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Capture {
            accuracy: R::Rcx,
            inverse_dodge: Source::Register(R::R8),
            caster: Caster::Mixed {
                reg: R::Rdi,
                other: R::Rsi,
                offset: STAT_PLAYER_SEED,
                shift: 20,
            },
            age_mixed: true,
        },
    },
    Site {
        rva: 0xed2d6d,
        bytes: &[0xc1, 0xe9, 0x11, 0x48, 0x01, 0xc1],
        kind: Kind::Complete {
            capture: 6,
            output: R::Rcx,
        },
    },
    // Accuracy producers: the value stored for the hit test.
    Site {
        rva: 0x12612c9,
        bytes: &[
            0x48, 0x6b, 0xc1, 0x46, 0x48, 0x89, 0x85, 0xa0, 0x00, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0xa0,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_PROJECTILE_CASTER,
            },
            trace: STAT_TRACE_PROJECTILE_KIND,
        },
    },
    Site {
        rva: 0x16f8d18,
        bytes: &[
            0x48, 0x6b, 0xc1, 0x46, 0x48, 0x89, 0x85, 0xd8, 0x02, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0x2d8,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_ENTITY_ID,
            },
            trace: STAT_TRACE_ACTION_STATE,
        },
    },
    Site {
        rva: 0x16f8e54,
        bytes: &[
            0x48, 0x6b, 0xc1, 0x46, 0x48, 0x89, 0x85, 0xd8, 0x02, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0x2d8,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_ENTITY_ID,
            },
            trace: STAT_TRACE_ACTION_STATE,
        },
    },
    // The Dodging producer of the input phase.
    Site {
        rva: 0x1466aff,
        bytes: &[0x49, 0x69, 0xce, 0x96, 0x00, 0x00, 0x00],
        kind: Kind::DodgingProducer {
            output: R::Rcx,
            entity: R::Rax,
        },
    },
    // The 10 shape-B collision consumers (clamp at zero): they add the shot's
    // Accuracy value, read from the operand named by `accuracy`, to the body
    // and subtract the target's Dodging.
    Site {
        rva: 0x103b444,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x103b694,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x12508c1,
        bytes: &[0x48, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbp,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1250b05,
        bytes: &[0x48, 0x2b, 0x96, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rsi,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1251002,
        bytes: &[0x49, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R13,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1251421,
        bytes: &[0x48, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbp,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x16f9622,
        bytes: &[0x49, 0x2b, 0x87, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R15,
            accuracy: (R::Rbp, 0x2d8),
        },
    },
    Site {
        rva: 0x16f9bd6,
        bytes: &[0x49, 0x2b, 0x84, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R12,
            accuracy: (R::Rbp, 0x2d8),
        },
    },
    Site {
        rva: 0x16f9d86,
        bytes: &[0x49, 0x2b, 0x97, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R15,
            accuracy: (R::Rbx, 0x8),
        },
    },
    Site {
        rva: 0x16fa420,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    // The 11 shape-A floors: `cmp output, 5001` right after
    // `sub output, [entity + dodging]`; the game then raises the margin to
    // 5000 (5 units) when it is lower.
    Site {
        rva: 0x103bac3,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x103bc0b,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x103c023,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x103c17b,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x103c675,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x103c8a2,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x103fed8,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x1040034,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x1262947,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::Rdi,
        },
    },
    Site {
        rva: 0x1262cdc,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R14,
        },
    },
    Site {
        rva: 0x1281050,
        bytes: &[0x48, 0x3d, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rax,
            entity: R::Rdi,
        },
    },
];
