//! Game 0.6.5: where the athlete stat rules act (see `crate::athlete_rules`,
//! docs/investigation-stat-rules-0.6.4.md and "Athlete stat rules" in
//! docs/migration-0.6.5.md). Every site replaces whole instructions that are
//! position independent; the hook replays them and then lets the rules adjust
//! registers or one frame slot. The verifier (tools/verify_native_profile.py)
//! decodes each site and checks the registers and offsets written here
//! against the executable.
//!
//! The game's Accuracy and Dodging here are effective values: each athlete's
//! condition factor applied, Accuracy pushed down further below 50. The
//! reaction gates roll their delay in a band and compare it at once, so they
//! use the `Inputs`/`Band` pair instead of `Capture`/`Complete`.
use crate::athlete_rules::{Caster, Dodging, Kind, Register as R, Site, Source};

/// Offset of the caster's actor id inside a projectile object.
pub(crate) const STAT_PROJECTILE_CASTER: usize = 0xf8;
/// Offset of the actor id inside a champion entity.
pub(crate) const STAT_ENTITY_ID: usize = 0x5b8;
/// Offset of the per-player seed the reaction gates mix into their hash.
pub(crate) const STAT_PLAYER_SEED: usize = 0x818;
/// Offset of the derived Dodging term inside an entity (Dodging x 200).
pub(crate) const STAT_DERIVED_DODGING: usize = 0x6a0;
/// The factor of that term: 200 (the Dodging producer's `imul`).
pub(crate) const STAT_DODGING_FACTOR: usize = 0xc8;
/// Offset of a projectile's kind, the word the hit code switches on after
/// the projectile Accuracy producer (traced by `STATRULE trace`).
pub(crate) const STAT_TRACE_PROJECTILE_KIND: usize = 0x40;
/// Offset of an entity's action state, the word the code switches on before
/// the entity Accuracy producers (traced by `STATRULE trace`).
pub(crate) const STAT_TRACE_ACTION_STATE: usize = 0x300;

pub(crate) const STAT_SITES: &[Site] = &[
    // projectile gate: the inputs where the game forms 100 - Dodging, then
    // the band at the roll.
    Site {
        rva: 0xfb5a81,
        bytes: &[0x41, 0xb8, 0x64, 0x00, 0x00, 0x00],
        kind: Kind::Inputs {
            accuracy: R::Rax,
            dodging: Dodging::Value(Source::Register(R::Rdi)),
            caster: Caster::Field {
                reg: R::Rbx,
                offset: STAT_PROJECTILE_CASTER,
            },
        },
    },
    Site {
        rva: 0xfb5b7c,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 0,
            seed: R::Rcx,
            low: R::R8,
            range: R::Rax,
            extra: R::R9,
        },
    },
    // big AI function, path 1: the inputs once Accuracy is final (Dodging
    // was worked out before the loop), then the band.
    Site {
        rva: 0xfa4cb9,
        bytes: &[0x89, 0xd0, 0x0f, 0xaf, 0xc0],
        kind: Kind::Inputs {
            accuracy: R::Rdx,
            dodging: Dodging::Inverse(Source::Frame(0x5a0)),
            caster: Caster::Field {
                reg: R::R14,
                offset: STAT_PROJECTILE_CASTER,
            },
        },
    },
    Site {
        rva: 0xfa4d81,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 2,
            seed: R::R8,
            low: R::R9,
            range: R::Rax,
            extra: R::Rcx,
        },
    },
    // big AI function, path 2: the same, the caster's id in a register.
    Site {
        rva: 0xfa50ea,
        bytes: &[0x89, 0xd0, 0x0f, 0xaf, 0xc0],
        kind: Kind::Inputs {
            accuracy: R::Rdx,
            dodging: Dodging::Inverse(Source::Frame(0x5a0)),
            caster: Caster::Shifted {
                reg: R::R13,
                shift: 0,
            },
        },
    },
    Site {
        rva: 0xfa51c1,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 4,
            seed: R::R9,
            low: R::R10,
            range: R::Rax,
            extra: R::R8,
        },
    },
    // area/casted-effect gate: the inputs where the game forms 100 -
    // Dodging, then the band.
    Site {
        rva: 0xf3843d,
        bytes: &[0xba, 0x64, 0x00, 0x00, 0x00],
        kind: Kind::Inputs {
            accuracy: R::Rax,
            dodging: Dodging::Value(Source::Register(R::R14)),
            caster: Caster::Mixed {
                reg: R::Rdi,
                other: R::Rsi,
                offset: STAT_PLAYER_SEED,
                shift: 20,
            },
        },
    },
    Site {
        rva: 0xf38537,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 6,
            seed: R::Rcx,
            low: R::R8,
            range: R::Rax,
            extra: R::R9,
        },
    },
    // the fifth gate (new in 0.6.5 to read Accuracy): the inputs before the
    // game mixes the caster's id with the seed, then the band.
    Site {
        rva: 0xfa1d2c,
        bytes: &[0x48, 0x8b, 0x89, 0xb8, 0x01, 0x00, 0x00],
        kind: Kind::Inputs {
            accuracy: R::Rax,
            dodging: Dodging::Value(Source::Register(R::Rsi)),
            caster: Caster::Shifted {
                reg: R::R14,
                shift: 0,
            },
        },
    },
    Site {
        rva: 0xfa1e57,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 8,
            seed: R::Rcx,
            low: R::R8,
            range: R::Rax,
            extra: R::R9,
        },
    },
    // Accuracy producers: the value stored for the hit test.
    Site {
        rva: 0x12fbc8b,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0xa8, 0x00, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0xa8,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_PROJECTILE_CASTER,
            },
            trace: STAT_TRACE_PROJECTILE_KIND,
        },
    },
    Site {
        rva: 0x1761657,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0xd8, 0x02, 0x00, 0x00,
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
        rva: 0x17617e2,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0xd8, 0x02, 0x00, 0x00,
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
    // The Dodging producer of the input phase (effective Dodging x 200).
    Site {
        rva: 0x1620b0d,
        bytes: &[0x48, 0x69, 0xce, 0xc8, 0x00, 0x00, 0x00],
        kind: Kind::DodgingProducer {
            output: R::Rcx,
            entity: R::Rax,
        },
    },
    // The 10 shape-B collision consumers (clamp at zero): they add the shot's
    // Accuracy value, read from the operand named by `accuracy`, to the body
    // and subtract the target's Dodging.
    Site {
        rva: 0x1050ec4,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1051114,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x12f3271,
        bytes: &[0x48, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbp,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x12f34b5,
        bytes: &[0x48, 0x2b, 0x96, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rsi,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x12f39b2,
        bytes: &[0x49, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R13,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x12f3dd1,
        bytes: &[0x48, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbp,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1761fb2,
        bytes: &[0x49, 0x2b, 0x87, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R15,
            accuracy: (R::Rbp, 0x2d8),
        },
    },
    Site {
        rva: 0x1762566,
        bytes: &[0x49, 0x2b, 0x84, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R12,
            accuracy: (R::Rbp, 0x2d8),
        },
    },
    Site {
        rva: 0x1762716,
        bytes: &[0x49, 0x2b, 0x97, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R15,
            accuracy: (R::Rbx, 0x8),
        },
    },
    Site {
        rva: 0x1762db0,
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
        rva: 0x1051543,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x105168b,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x1051aa3,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x1051bfb,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x10520f5,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x1052322,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x1055958,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x1055ab4,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x12fd2e7,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::Rdi,
        },
    },
    Site {
        rva: 0x12fd648,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R14,
        },
    },
    Site {
        rva: 0x1319520,
        bytes: &[0x48, 0x3d, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rax,
            entity: R::Rdi,
        },
    },
];
