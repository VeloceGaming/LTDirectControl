//! Game 0.6.5: the signal family (the ping orders since 1.7.1-dev13, the
//! research probe) is not reviewed for this executable: `SIGNALS` is not in
//! `ENABLED`, so these are never read, and the ping wheel sends nothing
//! here (a ping is the place). Each holds a value no address or field can have
//! (`usize::MAX - n`, zero bytes), never another version's value.

pub(crate) const SIGNAL_SITE: usize = usize::MAX - 1;
pub(crate) const SIGNAL_BYTES: [u8; 5] = [0; 5];
pub(crate) const SIGNAL_ORIGINAL: usize = usize::MAX - 2;
pub(crate) const SIGNAL_PROLOGUE: [u8; 19] = [0; 19];
pub(crate) const SIGNAL_TEAM_QUEUE_A: usize = usize::MAX - 3;
pub(crate) const SIGNAL_TEAM_QUEUE_B: usize = usize::MAX - 4;
pub(crate) const SIGNAL_TEAM_LIST_C: usize = usize::MAX - 5;
pub(crate) const SIGNAL_PLAYER_INDEX: usize = usize::MAX - 6;
pub(crate) const SIGNAL_PLAYER_SIDE: usize = usize::MAX - 7;
pub(crate) const SIGNAL_RECEIVER_MODE: usize = usize::MAX - 8;
pub(crate) const SIGNAL_RECEIVER_STATE: usize = usize::MAX - 10;
pub(crate) const SIGNAL_STATE_DELAY: usize = usize::MAX - 11;
pub(crate) const SIGNAL_JOIN_STATS: usize = usize::MAX - 12;
pub(crate) const SIGNAL_WORLD_TICK: usize = usize::MAX - 9;
pub(crate) const SIGNAL_PLAYER_ENTITY: usize = usize::MAX - 22;
/// No environment is known.
pub(crate) const SIGNAL_ENV_VTABLES: &[(usize, u8)] = &[];
pub(crate) const SIGNAL_LATE_SITE: usize = usize::MAX - 13;
pub(crate) const SIGNAL_LATE_BYTES: [u8; 5] = [0; 5];
pub(crate) const SIGNAL_LATE_ORIGINAL: usize = usize::MAX - 14;
pub(crate) const SIGNAL_LATE_PROLOGUE: [u8; 20] = [0; 20];
pub(crate) const SIGNAL_GATE_SITE: usize = usize::MAX - 15;
pub(crate) const SIGNAL_GATE_BYTES: [u8; 5] = [0; 5];
pub(crate) const SIGNAL_GATE_ORIGINAL: usize = usize::MAX - 16;
pub(crate) const SIGNAL_GATE_PROLOGUE: [u8; 19] = [0; 19];
/// No mode is accepted.
pub(crate) const SIGNAL_GATE_RULES: [(u16, bool); 5] = [(0, false); 5];
pub(crate) const SIGNAL_HEARD_SITES: [usize; 2] = [usize::MAX - 17, usize::MAX - 18];
pub(crate) const SIGNAL_HEARD_BYTES: [[u8; 5]; 2] = [[0; 5]; 2];
pub(crate) const SIGNAL_HEARD_ORIGINAL: usize = usize::MAX - 19;
pub(crate) const SIGNAL_HEARD_PROLOGUE: [u8; 19] = [0; 19];
pub(crate) const SIGNAL_SCENE_SITE: usize = usize::MAX - 20;
pub(crate) const SIGNAL_SCENE_BYTES: [u8; 5] = [0; 5];
pub(crate) const SIGNAL_SCENE_ORIGINAL: usize = usize::MAX - 21;
pub(crate) const SIGNAL_SCENE_PROLOGUE: [u8; 19] = [0; 19];
/// No mark, no scene reads (1.7.1-dev8).
pub(crate) const SIGNAL_MARK: u32 = 0;
pub(crate) const SIGNAL_SCENE_KIND: usize = usize::MAX - 23;
pub(crate) const SIGNAL_SCENE_ARG: usize = usize::MAX - 24;
pub(crate) const SIGNAL_SCENE_BLOCK: usize = usize::MAX - 25;
pub(crate) const SIGNAL_SCENE_TICK: usize = usize::MAX - 26;
pub(crate) const SIGNAL_SCENE_POS: usize = usize::MAX - 27;
pub(crate) const SIGNAL_SCENE_DIST: usize = usize::MAX - 28;
pub(crate) const SIGNAL_SCENE_FLAG: usize = usize::MAX - 29;
pub(crate) const SIGNAL_SIM_UNITS: usize = usize::MAX - 30;
pub(crate) const SIGNAL_UNIT_POS: usize = usize::MAX - 31;
pub(crate) const SIGNAL_UNIT_SPEED: usize = usize::MAX - 32;
pub(crate) const SIGNAL_UNIT_MAX_HP: usize = usize::MAX - 33;
pub(crate) const SIGNAL_UNIT_HP: usize = usize::MAX - 34;
/// No per-player levers (1.7.1-dev10).
pub(crate) const SIGNAL_TEAM_PLAN: usize = usize::MAX - 35;
pub(crate) const SIGNAL_OBJECTIVE: usize = usize::MAX - 36;
pub(crate) const SIGNAL_TRACE_ON: usize = usize::MAX - 37;
pub(crate) const SIGNAL_OBJECTIVE_SETTER: usize = usize::MAX - 38;
pub(crate) const SIGNAL_OBJECTIVE_PROLOGUE: [u8; 30] = [0; 30];
pub(crate) const SIGNAL_BIG_PLAN: usize = usize::MAX - 39;
pub(crate) const SIGNAL_BIG_PLAN_ARG: usize = usize::MAX - 40;
pub(crate) const SIGNAL_VISION_POS: usize = usize::MAX - 41;
pub(crate) const SIGNAL_VISION_SEEN: usize = usize::MAX - 42;
pub(crate) const SIGNAL_VISION_CHECKED: usize = usize::MAX - 43;
pub(crate) const SIGNAL_VISION_DISTANCE: usize = usize::MAX - 44;
pub(crate) const SIGNAL_OBJECTIVE_REASON: usize = usize::MAX - 45;
pub(crate) const SIGNAL_PLAN_LANES: &[(u64, usize)] = &[];
/// No ping orders on this version (1.7.1-dev13).
pub(crate) const SIGNAL_OBJECTIVE_REASON_TICK: usize = usize::MAX - 46;
/// No driven windows on this version (1.7.1-dev17).
pub(crate) const SIGNAL_PLAN_BATTLE: [u64; 2] = [u64::MAX; 2];
pub(crate) const SIGNAL_PLAN_PASSIVE_LINE: u64 = u64::MAX;
pub(crate) const SIGNAL_PLAN_HOLD_WAVE: usize = usize::MAX - 49;
pub(crate) const SIGNAL_PLAN_WAITING: usize = usize::MAX - 50;
pub(crate) const SIGNAL_PLAN_SPLIT_ROLE: usize = usize::MAX - 51;
pub(crate) const SIGNAL_PLAN_REGROUP_NOW: usize = usize::MAX - 52;
pub(crate) const SIGNAL_PLAN_PUSH_BASIS: usize = usize::MAX - 53;
pub(crate) const SIGNAL_BATTLE_RUNAWAY: usize = usize::MAX - 54;
pub(crate) const SIGNAL_BATTLE_RUNAWAY_TICK: usize = usize::MAX - 55;
pub(crate) const SIGNAL_SUPPORT_SLOT: u8 = u8::MAX;
pub(crate) const SIGNAL_SUPPORT_MODES: u16 = 0;
pub(crate) const SIGNAL_SUPPORT_SIM_WORDS: [usize; 2] = [usize::MAX - 56, usize::MAX - 57];
/// No take order play rules on this version (1.7.1-dev18).
pub(crate) const SIGNAL_SYNC_FOLLOWER: usize = usize::MAX - 58;
pub(crate) const SIGNAL_CONTEXT_SHARED: usize = usize::MAX - 59;
pub(crate) const SIGNAL_SHARED_STRIDE: usize = 0;
pub(crate) const SIGNAL_SHARED_SYNC: usize = usize::MAX - 60;
pub(crate) const SIGNAL_SHARED_RECORDS: [usize; 2] = [usize::MAX - 61, usize::MAX - 62];
pub(crate) const SIGNAL_BRAIN_RECORDS: [usize; 2] = [usize::MAX - 63, usize::MAX - 64];
pub(crate) const SIGNAL_RECORD_NONE: u8 = u8::MAX;
pub(crate) const SIGNAL_PLAN_PLAY: [u64; 2] = [u64::MAX; 2];
pub(crate) const SIGNAL_PLAN_REPLACE: usize = usize::MAX - 65;
pub(crate) const SIGNAL_PLAN_REPLACE_PROLOGUE: [u8; 51] = [0; 51];
pub(crate) const SIGNAL_FORCE_PASSIVE: u64 = u64::MAX;
pub(crate) const SIGNAL_PLAN_SIZE: usize = 0;
pub(crate) const SIGNAL_RECEIVER_WORLD: usize = usize::MAX - 66;
pub(crate) const SIGNAL_WORLD_OTHER_RALLY: usize = usize::MAX - 67;
pub(crate) const SIGNAL_RALLY: [usize; 2] = [usize::MAX - 68, usize::MAX - 69];
pub(crate) const SIGNAL_RALLY_STRIDE: usize = 0;
pub(crate) const SIGNAL_ENV_OBJECT: usize = usize::MAX - 70;
pub(crate) const SIGNAL_SPAWN_TICK: [usize; 2] = [usize::MAX - 71, usize::MAX - 72];
pub(crate) const SIGNAL_SERPEN_ALIVE: usize = usize::MAX - 73;
pub(crate) const SIGNAL_WALK_SLACK: u64 = 0;
