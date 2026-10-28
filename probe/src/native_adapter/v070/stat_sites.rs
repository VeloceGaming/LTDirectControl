//! Game 0.7.0: where the athlete stat rules act (see `crate::athlete_rules`,
//! "Athlete stat rules" in docs/migration-0.6.5.md and
//! docs/migration-0.7.0.md). Every site replaces whole instructions that are
//! position independent; the hook replays them and then lets the rules
//! adjust registers or one frame slot. The verifier
//! (tools/verify_native_profile.py) decodes each site and checks the
//! registers and offsets written here against the executable.
//!
//! As on 0.6.5 and Beta 2: effective Accuracy and Dodging, and reaction
//! gates that roll their delay in a band (`Inputs`/`Band`). Drafted from
//! Beta 2; the census found as many sites of each kind as Beta 2 has (10
//! hit tests, 11 floored hit tests, 3 Accuracy and 1 Dodging producers, 5
//! band gates).
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
        rva: 0x1126511,
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
        rva: 0x112660c,
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
        rva: 0x111554a,
        bytes: &[0x89, 0xd0, 0x0f, 0xaf, 0xc0],
        kind: Kind::Inputs {
            accuracy: R::Rdx,
            dodging: Dodging::Inverse(Source::Frame(0x588)),
            caster: Caster::Field {
                reg: R::R14,
                offset: STAT_PROJECTILE_CASTER,
            },
        },
    },
    Site {
        rva: 0x1115612,
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
        rva: 0x111595b,
        bytes: &[0x89, 0xd0, 0x0f, 0xaf, 0xc0],
        kind: Kind::Inputs {
            accuracy: R::Rdx,
            dodging: Dodging::Inverse(Source::Frame(0x588)),
            caster: Caster::Shifted {
                reg: R::Rsi,
                shift: 0,
            },
        },
    },
    Site {
        rva: 0x1115a32,
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
        rva: 0x109beed,
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
        rva: 0x109bfe7,
        bytes: &[0x48, 0xba, 0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e],
        kind: Kind::Band {
            inputs: 6,
            seed: R::Rcx,
            low: R::R8,
            range: R::Rax,
            extra: R::R9,
        },
    },
    // the fifth gate (reads Accuracy since 0.6.5): the inputs before the
    // game mixes the caster's id with the seed, then the band.
    Site {
        rva: 0x11125df,
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
        rva: 0x1112704,
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
        rva: 0x1d4b10d,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0x88, 0x00, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0x88,
            caster: Caster::Field {
                reg: R::R15,
                offset: STAT_PROJECTILE_CASTER,
            },
            trace: STAT_TRACE_PROJECTILE_KIND,
        },
    },
    Site {
        rva: 0x16703a7,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0xd0, 0x02, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0x2d0,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_ENTITY_ID,
            },
            trace: STAT_TRACE_ACTION_STATE,
        },
    },
    Site {
        rva: 0x1670526,
        bytes: &[
            0x48, 0x6b, 0xc0, 0x46, 0x48, 0x89, 0x85, 0xd0, 0x02, 0x00, 0x00,
        ],
        kind: Kind::AccuracyProducer {
            frame: 0x2d0,
            caster: Caster::Field {
                reg: R::Rsi,
                offset: STAT_ENTITY_ID,
            },
            trace: STAT_TRACE_ACTION_STATE,
        },
    },
    // The Dodging producer of the input phase (effective Dodging x 200).
    Site {
        rva: 0x18db0ed,
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
        rva: 0x1168ca4,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1168ef4,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1d3abc1,
        bytes: &[0x48, 0x2b, 0x95, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbp,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1d3ae05,
        bytes: &[0x48, 0x2b, 0x96, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rsi,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1d3b0c7,
        bytes: &[0x48, 0x2b, 0x93, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::Rbx,
            accuracy: (R::Rdi, 0x8),
        },
    },
    Site {
        rva: 0x1d3b336,
        bytes: &[0x49, 0x2b, 0x94, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R12,
            accuracy: (R::Rax, 0x8),
        },
    },
    Site {
        rva: 0x1670d12,
        bytes: &[0x49, 0x2b, 0x87, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R15,
            accuracy: (R::Rbp, 0x2d0),
        },
    },
    Site {
        rva: 0x1671316,
        bytes: &[0x49, 0x2b, 0x84, 0x24, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rax,
            entity: R::R12,
            accuracy: (R::Rbp, 0x2d0),
        },
    },
    Site {
        rva: 0x167166d,
        bytes: &[0x49, 0x2b, 0x97, 0xa0, 0x06, 0x00, 0x00],
        kind: Kind::Collision {
            output: R::Rdx,
            entity: R::R15,
            accuracy: (R::Rbx, 0x8),
        },
    },
    Site {
        rva: 0x1671d30,
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
        rva: 0x1169323,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x116946b,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x1169883,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x11699db,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x1169ed5,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x116a102,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x116e558,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R15,
        },
    },
    Site {
        rva: 0x116e6b4,
        bytes: &[0x49, 0x81, 0xfe, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::R14,
            entity: R::R13,
        },
    },
    Site {
        rva: 0x1d4c5c7,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::Rbx,
        },
    },
    Site {
        rva: 0x1d4c874,
        bytes: &[0x48, 0x81, 0xf9, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rcx,
            entity: R::R12,
        },
    },
    Site {
        rva: 0x1d69b20,
        bytes: &[0x48, 0x3d, 0x89, 0x13, 0x00, 0x00],
        kind: Kind::FlooredCollision {
            output: R::Rax,
            entity: R::Rdi,
        },
    },
];
