//! What a ping means (1.7.1-dev17; "here" versus "that" since
//! 1.7.1-dev15): the author's spec section 10.2. Pure functions.
//!
//! The wheel speaks about *here*, the place at the press point: a pit
//! while its monster is alive, the own base, a lane, or the ground (with
//! the nearest enemy champion within [`NEAR_ENEMY`], if any). Its eight
//! slots never change: the right half is personal, the left the team's.
//!
//! | Slot | Order |
//! | --- | --- |
//! | Assist me (right, up) | the fight call on the enemy champion nearest the **player** within `NEAR_ENEMY`, to everyone; none: a mark at the player |
//! | On my way (right, up-and-out) | hold the wave: a driven window on the laner of the point's lane nearest the point (`ping_orders::Drive::Hold`) |
//! | Enemy missing (right, down-and-out) | the fog ping's vision write to everyone, and a regroup window on every other lane's laners ([`Team::others`]) |
//! | Retreat (right, down) | a retreat window on the allies within [`RETREAT_NEAR`] of the point |
//! | Take here (left, up) | pit: Setup without battle; lane: Nexus(lane); base: push its nearest lane; ground: the fight call on a near enemy, else a mark |
//! | Defend here (left, up-and-out) | pit: Setup with battle; lane: DefenseLine(lane); base: Defense; ground: a mark |
//! | Poke (left, down-and-out) | the team strategy's window (`plan_call`): `object_battle` and `tower_press` Poking |
//! | All-in (left, down) | the team strategy's window: `object_battle` Initiating, `object_buildup` Gather |
//!
//! Lane orders go to that lane's laners, or everyone when it is the
//! controlled athlete's own lane ([`Team::crew`]); pit orders to everyone.
//! "The point's lane" is the lane there, else the nearest lane.
//!
//! The armed click speaks about *that*, the unit under the cursor at the
//! click ([`Clicked`]); with none it is Take here's reading of the ground.
//!
//! | Click on | Order |
//! | --- | --- |
//! | an enemy champion the team sees | the fight call |
//! | an enemy the team cannot see | the fog ping at the click point (the vision write) |
//! | a pit monster, alive | take: Setup without battle, to all |
//! | a tower | push its lane: Nexus to its laner |
//! | an ally | nothing yet (reserved) |
//! | anything else or nothing | Take here at the click point |
//!
//! A choice nobody can hear, or that this game version cannot send, marks
//! the place: nothing greys and nothing is lost.
//!
//! Superseded 2026-11-04 (dev17): dev15 and dev16's four slots (go,
//! defend, missing, careful), go's and defend's variants (with a fight,
//! everyone), careful's stop call, I'm coming's hide-lane call and the fog
//! ping's Mia call (inert in the viewed match's AI kind).
use super::places::{self, Pit};
use crate::native_adapter::ping_orders::{Drive, Objective, Send};
use crate::native_adapter::PingReach;

/// World units.
pub type Point = (u64, u64);

/// An enemy champion within this many map units of a ground point (or of
/// the player, for Assist me) gets the fight call (the author's spec,
/// "about 300").
pub const NEAR_ENEMY: f32 = 300.;
/// Retreat reaches the allies within this many map units of the point:
/// the radius the game uses for "an enemy near the tower"
/// (docs/investigation-team-ai.md section 15), the same as `NEAR_ENEMY`.
pub const RETREAT_NEAR: f32 = 300.;

/// The wheel's eight slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    AssistMe,
    OnMyWay,
    EnemyMissing,
    Retreat,
    TakeHere,
    DefendHere,
    Poke,
    AllIn,
}
impl Slot {
    /// By half (right personal, left team) and the gesture's slot order:
    /// up, up-and-out, down-and-out, down.
    pub const HALVES: [[Slot; 4]; 2] = [
        [
            Slot::AssistMe,
            Slot::OnMyWay,
            Slot::EnemyMissing,
            Slot::Retreat,
        ],
        [Slot::TakeHere, Slot::DefendHere, Slot::Poke, Slot::AllIn],
    ];
    pub fn at(half: usize, n: usize) -> Option<Slot> {
        Self::HALVES.get(half)?.get(n).copied()
    }
    /// Its name, the tooltip's words.
    pub fn word(self) -> &'static str {
        match self {
            Slot::AssistMe => crate::lang::tr("Assist me"),
            Slot::OnMyWay => crate::lang::tr("On my way"),
            Slot::EnemyMissing => crate::lang::tr("Enemy missing"),
            Slot::Retreat => crate::lang::tr("Retreat"),
            Slot::TakeHere => crate::lang::tr("Take here"),
            Slot::DefendHere => crate::lang::tr("Defend here"),
            Slot::Poke => crate::lang::tr("Poke"),
            Slot::AllIn => crate::lang::tr("All-in"),
        }
    }
    /// Its glyph on the fan.
    pub fn glyph(self) -> Glyph {
        match self {
            Slot::AssistMe => Glyph::Assist,
            Slot::OnMyWay => Glyph::Coming,
            Slot::EnemyMissing => Glyph::Missing,
            Slot::Retreat => Glyph::Leave,
            Slot::TakeHere => Glyph::Take,
            Slot::DefendHere => Glyph::Defend,
            Slot::Poke => Glyph::Poke,
            Slot::AllIn => Glyph::AllIn,
        }
    }
    pub fn log_name(self) -> &'static str {
        match self {
            Slot::AssistMe => "assist_me",
            Slot::OnMyWay => "on_my_way",
            Slot::EnemyMissing => "enemy_missing",
            Slot::Retreat => "retreat",
            Slot::TakeHere => "take_here",
            Slot::DefendHere => "defend_here",
            Slot::Poke => "poke",
            Slot::AllIn => "all_in",
        }
    }
    /// The place word the tooltip adds (`Take here · Serpen`), where the
    /// slot reads the place.
    pub fn place_word(self, place: Place) -> Option<&'static str> {
        match self {
            Slot::TakeHere | Slot::DefendHere => match place {
                Place::Pit(p) if p.tag == Objective::SERPEN => Some(crate::lang::tr("Serpen")),
                Place::Pit(_) => Some(crate::lang::tr("Morgard")),
                Place::Lane { lane, .. } => Some(lane_word(lane)),
                Place::Base(_) => Some(crate::lang::tr("base")),
                Place::Ground { .. } => None,
            },
            Slot::OnMyWay | Slot::EnemyMissing => Some(lane_word(place.lane())),
            _ => None,
        }
    }
}
/// A lane's word in the tooltip (its own keys: "Top", "Mid" and "Bottom"
/// name roles elsewhere, and some languages word them so).
pub fn lane_word(lane: u8) -> &'static str {
    match lane {
        0 => crate::lang::tr("Top lane"),
        1 => crate::lang::tr("Mid lane"),
        _ => crate::lang::tr("Bottom lane"),
    }
}

/// The pixel glyphs (design/pings/glyphs-pixel.svg and glyphs-pixel-extra.svg,
/// matched by their labels; pictures: `tools/generate_ping_assets.py`,
/// `wheel_draw::stem`). The fight call's focus has no glyph of its own: it
/// shows the crossed swords, contest's; "all in" shows them too until the
/// author's sheet has an `all in` label (the generator takes it by label).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Go,
    Defend,
    Missing,
    Take,
    Contest,
    Leave,
    Push,
    Coming,
    Accepted,
    Busy,
    Declined,
    Assist,
    Poke,
    AllIn,
    /// The halves' markers: personal (me), team (everyone).
    Personal,
    Team,
}
impl Glyph {
    /// The glyph's one colour (0xRRGGBBAA), the answer stack's top line
    /// (the generator's record keeps the same values; a test compares).
    pub fn color(self) -> u32 {
        match self {
            Glyph::Go | Glyph::Coming | Glyph::Accepted => 0x5bd16eff,
            Glyph::Defend => 0x4fa3e6ff,
            Glyph::Missing | Glyph::Busy => 0xfdee00ff,
            Glyph::Leave | Glyph::Declined => 0xff4d5eff,
            Glyph::Take => 0xb77cffff,
            Glyph::Contest | Glyph::AllIn | Glyph::Poke => 0xff9640ff,
            Glyph::Push => 0xff7ac8ff,
            Glyph::Assist => 0x46d6d0ff,
            Glyph::Personal | Glyph::Team => 0xeeececff,
        }
    }
}

/// An enemy champion: his entity id and, once known, his player index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enemy {
    pub entity: usize,
    pub index: Option<u8>,
}
/// Where the wheel's press was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Pit(Pit),
    Base(Point),
    /// A lane, with the nearest enemy the team sees.
    Lane {
        lane: u8,
        at: Point,
        enemy: Option<Enemy>,
    },
    /// Plain ground, with the nearest enemy champion within
    /// [`NEAR_ENEMY`].
    Ground {
        at: Point,
        enemy: Option<Enemy>,
    },
}
impl Place {
    pub fn at(self) -> Point {
        match self {
            Place::Pit(p) => p.at,
            Place::Base(at) | Place::Lane { at, .. } | Place::Ground { at, .. } => at,
        }
    }
    /// The point's lane: the lane there, else the nearest lane.
    pub fn lane(self) -> u8 {
        match self {
            Place::Lane { lane, .. } => lane,
            other => places::nearest_lane(other.at()),
        }
    }
    pub fn text(self) -> String {
        match self {
            Place::Pit(p) => format!(
                "pit={} at={},{}",
                if p.tag == 0 { "Morgard" } else { "Serpen" },
                p.at.0,
                p.at.1
            ),
            Place::Base(at) => format!("base at={},{}", at.0, at.1),
            Place::Lane { lane, at, .. } => format!("lane={lane} at={},{}", at.0, at.1),
            Place::Ground { at, enemy } => format!(
                "ground at={},{} enemy={}",
                at.0,
                at.1,
                enemy.map_or("none".to_owned(), |e| e.entity.to_string())
            ),
        }
    }
}
/// What the armed click landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clicked {
    Enemy(Enemy),
    /// A live pit's monster (its entity) and its pit.
    Monster(usize, Pit),
    /// A tower (its entity) and the lane it stands on.
    Tower(usize, u8),
    Ally(usize),
    /// A unit the team cannot see (honest mechanics: no knowledge the team
    /// lacks): the fog ping at the point, with the lane there if any.
    Fog(Point, Option<u8>),
    /// Nothing, or a unit without a ping of its own: the ground there.
    Ground(Place),
}
impl Clicked {
    pub fn text(self) -> String {
        match self {
            Clicked::Enemy(e) => format!(
                "enemy={} index={}",
                e.entity,
                e.index.map_or("?".to_owned(), |i| i.to_string())
            ),
            Clicked::Monster(id, p) => format!(
                "monster={id} pit={}",
                if p.tag == 0 { "Morgard" } else { "Serpen" }
            ),
            Clicked::Tower(id, lane) => format!("tower={id} lane={lane}"),
            Clicked::Ally(id) => format!("ally={id}"),
            Clicked::Fog(at, lane) => format!(
                "fog at={},{} lane={}",
                at.0,
                at.1,
                lane.map_or("none".to_owned(), |l| l.to_string())
            ),
            Clicked::Ground(place) => place.text(),
        }
    }
}

/// Who can hear: the controlled athlete's team by brain index (roster slot
/// 0 top, 1 jungle, 2 mid, 3 and 4 bottom).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Team {
    /// The controlled athlete's own index (never a recipient).
    pub me: Option<u8>,
    /// Each teammate's champion position (the controlled athlete's too),
    /// while he lives and is known.
    pub pos: [Option<Point>; 5],
    pub reach: PingReach,
}
pub type Mask = [bool; 5];
fn gap(a: Point, b: Point) -> f32 {
    (a.0 as f32 - b.0 as f32).hypot(a.1 as f32 - b.1 as f32) / 1000.
}
impl Team {
    fn hears(&self, i: usize) -> bool {
        self.pos[i].is_some() && self.me != Some(i as u8)
    }
    /// Every teammate who can hear.
    fn everyone(&self) -> Mask {
        std::array::from_fn(|i| self.hears(i))
    }
    /// The controlled athlete's position.
    pub fn me_at(&self) -> Option<Point> {
        self.pos[usize::from(self.me?).min(4)]
    }
    /// The controlled athlete is that lane's laner.
    fn mine(&self, lane: u8) -> bool {
        self.me
            .and_then(crate::native_adapter::ping_orders::own_lane)
            == Some(lane)
    }
    /// Who a lane order goes to (1.7.1-dev16): that lane's laners, or
    /// everyone when the lane is the controlled athlete's own.
    fn crew(&self, lane: u8) -> Mask {
        if self.mine(lane) {
            self.everyone()
        } else {
            self.laners(lane)
        }
    }
    /// That lane's laners (bottom has two), never the jungler.
    fn laners(&self, lane: u8) -> Mask {
        std::array::from_fn(|i| {
            self.hears(i) && crate::native_adapter::ping_orders::own_lane(i as u8) == Some(lane)
        })
    }
    /// The laner of `lane` nearest `at` (On my way).
    fn nearest_laner(&self, lane: u8, at: Point) -> Mask {
        let laners = self.laners(lane);
        let best = (0..5)
            .filter(|i| laners[*i])
            .filter_map(|i| Some((i, gap(self.pos[i]?, at))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        std::array::from_fn(|i| Some(i) == best)
    }
    /// Every laner but `lane`'s (1.7.1-dev18, spec section 11.4): roster
    /// slots 0, 2, 3, 4 off that lane, never the jungler or the player.
    /// Superseded 2026-11-04: dev17's neighbouring lanes only (mid's were
    /// top and bottom, a side lane's mid), which left a side lane's ping
    /// unheard on the other side lane.
    fn others(&self, lane: u8) -> Mask {
        std::array::from_fn(|i| {
            self.hears(i)
                && crate::native_adapter::ping_orders::own_lane(i as u8).is_some_and(|l| l != lane)
        })
    }
    /// The allies within `RETREAT_NEAR` of `at`.
    fn near(&self, at: Point) -> Mask {
        std::array::from_fn(|i| {
            self.hears(i) && self.pos[i].is_some_and(|p| gap(p, at) <= RETREAT_NEAR)
        })
    }
}

/// A team strategy window (`plan_call`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanCall {
    Poke,
    AllIn,
    /// `object_finish` KillPriority: Take here at a pit, the click on a
    /// pit monster (1.7.1-dev18, spec section 11.5).
    KillFirst,
    /// `object_finish` BattlePriority: Defend here at a pit.
    FightFirst,
}
impl PlanCall {
    /// Its name in the log.
    pub fn text(self) -> &'static str {
        match self {
            PlanCall::Poke => "poke",
            PlanCall::AllIn => "all_in",
            PlanCall::KillFirst => "kill_first",
            PlanCall::FightFirst => "fight_first",
        }
    }
}
/// The finishing rule a pit order also sets (spec section 11.5): Take
/// (Setup without battle) kills first, Defend (with battle) fights first;
/// None for any other order, or where this version sends no team plan
/// (0.6.5: `reach.calls` off, as for Poke and All-in). The fight plan's
/// "objective priority" exit reads `object_finish`
/// (docs/investigation-team-ai.md section 17, exit 4).
pub fn finish(order: Order, reach: PingReach) -> Option<PlanCall> {
    let Order::Send(Send::Objective(o), _) = order else {
        return None;
    };
    let battle = o.pit_battle().filter(|_| reach.calls)?;
    Some(if battle {
        PlanCall::FightFirst
    } else {
        PlanCall::KillFirst
    })
}

/// What a choice does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// The fight call on an enemy champion (entity id), to every teammate.
    Fight(usize),
    Send(Send, Mask),
    /// A driven window (`ping_orders::Drive`) on these teammates.
    Drive(Drive, Mask),
    /// Enemy missing: the fog ping's vision write naming this enemy (None:
    /// the worker picks) to `fog`, and the regroup window on `regroup`.
    Missing {
        enemy: Option<u8>,
        fog: Mask,
        regroup: Mask,
    },
    /// The armed click's fog ping: the vision write only.
    Fog(Option<u8>, Mask),
    /// A team strategy window; the mask says whose rows the stack shows.
    Plan(PlanCall, Mask),
    /// Only the mark at the place.
    Mark,
}
/// A choice: its order and the glyph the pin and the stack show (the
/// order's, so the player sees how "here" was understood).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub glyph: Glyph,
    pub order: Order,
}

/// The lane's enemy laner by roster slot (lane 0 slot 0, 1 slot 2, 2 slot
/// 3), for a fog ping on a lane.
fn enemy_laner(lane: u8) -> u8 {
    match lane {
        0 => 0,
        1 => 2,
        _ => 3,
    }
}

/// A choice that sends, when someone can hear it and this version can send
/// it; else the mark with the same glyph.
fn send(team: &Team, glyph: Glyph, send: Send, to: Mask) -> Choice {
    let reachable = match send {
        Send::Fog(_) => team.reach.calls,
        Send::Objective(_) => team.reach.objectives,
    };
    let order = if reachable && to.contains(&true) {
        Order::Send(send, to)
    } else {
        Order::Mark
    };
    Choice { glyph, order }
}
fn mark(glyph: Glyph) -> Choice {
    Choice {
        glyph,
        order: Order::Mark,
    }
}
/// A driven window on `to`, when someone can hear it.
fn drive(team: &Team, glyph: Glyph, drive: Drive, to: Mask) -> Choice {
    let order = if team.reach.calls && to.contains(&true) {
        Order::Drive(drive, to)
    } else {
        Order::Mark
    };
    Choice { glyph, order }
}
/// The fog ping (the armed click on an enemy the team cannot see), naming
/// the lane's enemy laner on a lane.
fn fog(team: &Team, lane: Option<u8>) -> Choice {
    let to = team.everyone();
    let order = if team.reach.calls && to.contains(&true) {
        Order::Fog(lane.map(enemy_laner), to)
    } else {
        Order::Mark
    };
    Choice {
        glyph: Glyph::Missing,
        order,
    }
}
fn fight(team: &Team, entity: usize, glyph: Glyph) -> Choice {
    let order = if team.reach.calls && team.everyone().contains(&true) {
        Order::Fight(entity)
    } else {
        Order::Mark
    };
    Choice { glyph, order }
}
/// A team strategy window, where this version sends the pings' orders at
/// all (0.6.5: the place only, as every other order).
fn plan(team: &Team, glyph: Glyph, call: PlanCall) -> Choice {
    let order = if team.reach.calls {
        Order::Plan(call, team.everyone())
    } else {
        Order::Mark
    };
    Choice { glyph, order }
}
/// Take here on a lane: Nexus(lane) to its crew.
fn push(team: &Team, lane: u8) -> Choice {
    let nexus = Send::Objective(Objective::with_lane(Objective::NEXUS, lane));
    send(team, Glyph::Push, nexus, team.crew(lane))
}

/// What the wheel knows at its press: the place, and the enemy champion
/// nearest the player within `NEAR_ENEMY` (for Assist me).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Here {
    pub place: Place,
    pub near_me: Option<Enemy>,
}

/// The wheel's choice of `slot` at `here`.
pub fn wheel(here: Here, slot: Slot, team: &Team) -> Choice {
    let place = here.place;
    let all = team.everyone();
    match slot {
        Slot::AssistMe => match here.near_me {
            Some(e) => fight(team, e.entity, Glyph::Assist),
            None => mark(Glyph::Assist),
        },
        Slot::OnMyWay => {
            let lane = place.lane();
            let hold = Drive::Hold { lane };
            drive(
                team,
                Glyph::Coming,
                hold,
                team.nearest_laner(lane, place.at()),
            )
        }
        Slot::EnemyMissing => {
            let regroup = team.others(place.lane());
            let enemy = match place {
                Place::Lane { lane, .. } => Some(enemy_laner(lane)),
                _ => None,
            };
            let order = if team.reach.calls && all.contains(&true) {
                Order::Missing {
                    enemy,
                    fog: all,
                    regroup,
                }
            } else {
                Order::Mark
            };
            Choice {
                glyph: Glyph::Missing,
                order,
            }
        }
        Slot::Retreat => drive(team, Glyph::Leave, Drive::Retreat, team.near(place.at())),
        Slot::TakeHere => match place {
            Place::Pit(pit) => send(
                team,
                Glyph::Take,
                Send::Objective(Objective::pit(pit.tag, Objective::SETUP, false)),
                all,
            ),
            Place::Lane { lane, .. } => push(team, lane),
            Place::Base(at) => push(team, places::nearest_lane(at)),
            Place::Ground { enemy: Some(e), .. } => fight(team, e.entity, Glyph::Contest),
            Place::Ground { .. } => mark(Glyph::Go),
        },
        Slot::DefendHere => match place {
            Place::Pit(pit) => send(
                team,
                Glyph::Contest,
                Send::Objective(Objective::pit(pit.tag, Objective::SETUP, true)),
                all,
            ),
            Place::Lane { lane, .. } => send(
                team,
                Glyph::Defend,
                Send::Objective(Objective::with_lane(Objective::DEFENSE_LINE, lane)),
                team.crew(lane),
            ),
            Place::Base(_) => send(
                team,
                Glyph::Defend,
                Send::Objective(Objective::DEFENSE),
                all,
            ),
            Place::Ground { .. } => mark(Glyph::Defend),
        },
        Slot::Poke => plan(team, Glyph::Poke, PlanCall::Poke),
        Slot::AllIn => plan(team, Glyph::AllIn, PlanCall::AllIn),
    }
}

/// The armed click's choice on what it landed on; None for an ally
/// (nothing yet).
pub fn click(on: Clicked, team: &Team) -> Option<Choice> {
    let all = team.everyone();
    Some(match on {
        Clicked::Enemy(e) => fight(team, e.entity, Glyph::Contest),
        Clicked::Monster(_, pit) => send(
            team,
            Glyph::Take,
            Send::Objective(Objective::pit(pit.tag, Objective::SETUP, false)),
            all,
        ),
        Clicked::Tower(_, lane) => push(team, lane),
        Clicked::Ally(_) => return None,
        Clicked::Fog(_, lane) => fog(team, lane),
        Clicked::Ground(place) => wheel(
            Here {
                place,
                near_me: None,
            },
            Slot::TakeHere,
            team,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const REACH: PingReach = PingReach {
        calls: true,
        objectives: true,
    };
    fn team(me: u8) -> Team {
        Team {
            me: Some(me),
            pos: std::array::from_fn(|i| Some((i as u64 * 100_000, 500_000))),
            reach: REACH,
        }
    }
    fn mask(on: &[usize]) -> Mask {
        std::array::from_fn(|i| on.contains(&i))
    }
    fn lane(lane: u8) -> Place {
        Place::Lane {
            lane,
            at: (90_000, 500_000),
            enemy: None,
        }
    }
    fn here(place: Place) -> Here {
        Here {
            place,
            near_me: None,
        }
    }
    const SERPEN: Pit = Pit {
        tag: Objective::SERPEN,
        at: (610_000, 610_000),
    };
    const ALL: [Glyph; 16] = [
        Glyph::Go,
        Glyph::Defend,
        Glyph::Missing,
        Glyph::Take,
        Glyph::Contest,
        Glyph::Leave,
        Glyph::Push,
        Glyph::Coming,
        Glyph::Accepted,
        Glyph::Busy,
        Glyph::Declined,
        Glyph::Assist,
        Glyph::Poke,
        Glyph::AllIn,
        Glyph::Personal,
        Glyph::Team,
    ];

    #[test]
    fn each_glyph_colour_is_the_one_the_pixel_art_has() {
        let record: serde_json::Value =
            serde_json::from_str(include_str!("../../../tools/records/ping-assets.json")).unwrap();
        for g in ALL {
            let name = super::super::wheel_draw::stem(g);
            let hex = record["colours"][name].as_str().unwrap();
            assert_eq!(format!("#{:06x}", g.color() >> 8), hex, "{name}");
        }
    }

    #[test]
    fn the_eight_slots_sit_on_their_halves_with_their_words() {
        assert_eq!(Slot::at(0, 1), Some(Slot::OnMyWay));
        assert_eq!(Slot::at(1, 3), Some(Slot::AllIn));
        assert_eq!(Slot::at(2, 0), None);
        assert_eq!(
            Slot::TakeHere.place_word(Place::Pit(SERPEN)),
            Some(crate::lang::tr("Serpen"))
        );
        assert_eq!(
            Slot::OnMyWay.place_word(lane(2)),
            Some(crate::lang::tr("Bottom lane"))
        );
        assert_eq!(Slot::Retreat.place_word(lane(2)), None);
        let ground = Place::Ground {
            at: (1, 1),
            enemy: None,
        };
        assert_eq!(Slot::TakeHere.place_word(ground), None);
    }

    #[test]
    fn take_and_defend_read_the_lane_with_its_laners_or_everyone() {
        // Bottom lane, from the mid laner (2): bottom's two laners.
        let t = team(2);
        let bottom = mask(&[3, 4]);
        let all = mask(&[0, 1, 3, 4]);
        let choice = |s| wheel(here(lane(2)), s, &t);
        assert_eq!(
            choice(Slot::TakeHere),
            Choice {
                glyph: Glyph::Push,
                order: Order::Send(Send::Objective(Objective::with_lane(4, 2)), bottom)
            }
        );
        assert_eq!(
            choice(Slot::DefendHere).order,
            Order::Send(Send::Objective(Objective::with_lane(3, 2)), bottom)
        );
        // Mid from the mid laner: his own lane's orders go to everyone.
        assert_eq!(
            wheel(here(lane(1)), Slot::TakeHere, &t).order,
            Order::Send(Send::Objective(Objective::with_lane(4, 1)), all)
        );
        // The pit: take, contest; the base: defense to all, push nearest.
        let pit = |s| wheel(here(Place::Pit(SERPEN)), s, &t);
        assert_eq!(
            pit(Slot::TakeHere),
            Choice {
                glyph: Glyph::Take,
                order: Order::Send(Send::Objective(Objective::pit(1, 1, false)), all)
            }
        );
        assert_eq!(
            pit(Slot::DefendHere),
            Choice {
                glyph: Glyph::Contest,
                order: Order::Send(Send::Objective(Objective::pit(1, 1, true)), all)
            }
        );
        let base = here(Place::Base((20_000, 915_000)));
        assert_eq!(
            wheel(base, Slot::DefendHere, &t).order,
            Order::Send(Send::Objective(Objective::DEFENSE), all)
        );
        assert_eq!(
            wheel(base, Slot::TakeHere, &t).order,
            Order::Send(Send::Objective(Objective::with_lane(4, 2)), bottom)
        );
    }

    #[test]
    fn the_ground_calls_the_fight_on_a_near_enemy_and_else_marks() {
        let t = team(0);
        let enemy = Enemy {
            entity: 25,
            index: Some(3),
        };
        let near = Place::Ground {
            at: (1, 1),
            enemy: Some(enemy),
        };
        assert_eq!(
            wheel(here(near), Slot::TakeHere, &t),
            Choice {
                glyph: Glyph::Contest,
                order: Order::Fight(25)
            }
        );
        assert_eq!(wheel(here(near), Slot::DefendHere, &t), mark(Glyph::Defend));
        let empty = Place::Ground {
            at: (1, 1),
            enemy: None,
        };
        assert_eq!(wheel(here(empty), Slot::TakeHere, &t), mark(Glyph::Go));
    }

    #[test]
    fn assist_me_calls_the_fight_near_the_player_and_else_marks_him() {
        let t = team(2);
        let near_me = Some(Enemy {
            entity: 31,
            index: None,
        });
        let at = Here {
            place: lane(0),
            near_me,
        };
        assert_eq!(
            wheel(at, Slot::AssistMe, &t),
            Choice {
                glyph: Glyph::Assist,
                order: Order::Fight(31)
            }
        );
        assert_eq!(
            wheel(here(lane(0)), Slot::AssistMe, &t),
            mark(Glyph::Assist)
        );
        assert_eq!(t.me_at(), Some((200_000, 500_000)));
    }

    #[test]
    fn on_my_way_holds_the_nearest_laner_of_the_points_lane() {
        let t = team(2);
        // Bottom: laners 3 (x 300) and 4 (x 400); the point at x 390.
        let point = Place::Lane {
            lane: 2,
            at: (390_000, 500_000),
            enemy: None,
        };
        assert_eq!(
            wheel(here(point), Slot::OnMyWay, &t),
            Choice {
                glyph: Glyph::Coming,
                order: Order::Drive(Drive::Hold { lane: 2 }, mask(&[4]))
            }
        );
        // Never the jungler, never the player: his own lane has no other
        // laner on mid.
        assert_eq!(wheel(here(lane(1)), Slot::OnMyWay, &t), mark(Glyph::Coming));
        // Off-lane: the nearest lane (bottom, by the bottom base).
        let off = Place::Ground {
            at: (20_000, 915_000),
            enemy: None,
        };
        assert_eq!(off.lane(), 2);
        assert!(matches!(
            wheel(here(off), Slot::OnMyWay, &t).order,
            Order::Drive(Drive::Hold { lane: 2 }, _)
        ));
    }

    #[test]
    fn enemy_missing_regroups_every_other_laner_and_writes_vision_for_all() {
        let regroup =
            |me: u8, l: u8| match wheel(here(lane(l)), Slot::EnemyMissing, &team(me)).order {
                Order::Missing { regroup, .. } => regroup,
                other => panic!("{other:?}"),
            };
        // The player top (0): mid's ping reaches bottom, top's mid and
        // bottom, bottom's mid; never the jungler (1) or the player.
        assert_eq!(regroup(0, 1), mask(&[3, 4]));
        assert_eq!(regroup(0, 0), mask(&[2, 3, 4]));
        assert_eq!(regroup(0, 2), mask(&[2]));
        // The player mid (2): a side lane's ping reaches the other side.
        assert_eq!(regroup(2, 0), mask(&[3, 4]));
        assert_eq!(regroup(2, 2), mask(&[0]));
        assert_eq!(
            wheel(here(lane(1)), Slot::EnemyMissing, &team(0)).order,
            Order::Missing {
                enemy: Some(2),
                fog: mask(&[1, 2, 3, 4]),
                regroup: mask(&[3, 4]),
            }
        );
    }

    #[test]
    fn retreat_reaches_the_allies_near_the_point() {
        let t = team(0);
        // Allies at x 100, 200, 300, 400; the point at x 250: all four
        // within 300.
        let point = Place::Ground {
            at: (250_000, 500_000),
            enemy: None,
        };
        assert_eq!(
            wheel(here(point), Slot::Retreat, &t).order,
            Order::Drive(Drive::Retreat, mask(&[1, 2, 3, 4]))
        );
        let far = Place::Ground {
            at: (250_000, 100_000),
            enemy: None,
        };
        assert_eq!(wheel(here(far), Slot::Retreat, &t), mark(Glyph::Leave));
    }

    #[test]
    fn poke_and_all_in_are_team_plans() {
        let t = team(0);
        assert_eq!(
            wheel(here(lane(1)), Slot::Poke, &t).order,
            Order::Plan(PlanCall::Poke, mask(&[1, 2, 3, 4]))
        );
        assert_eq!(wheel(here(lane(1)), Slot::AllIn, &t).glyph, Glyph::AllIn);
    }

    #[test]
    fn take_and_defend_at_a_pit_set_the_finishing_rule() {
        let t = team(0);
        let pit = |s| wheel(here(Place::Pit(SERPEN)), s, &t).order;
        assert_eq!(
            finish(pit(Slot::TakeHere), REACH),
            Some(PlanCall::KillFirst)
        );
        assert_eq!(
            finish(pit(Slot::DefendHere), REACH),
            Some(PlanCall::FightFirst)
        );
        let monster = click(Clicked::Monster(40, SERPEN), &t).unwrap().order;
        assert_eq!(finish(monster, REACH), Some(PlanCall::KillFirst));
        // A lane, the base, a plan: none.
        for s in [Slot::TakeHere, Slot::DefendHere, Slot::Poke] {
            assert_eq!(finish(wheel(here(lane(1)), s, &t).order, REACH), None);
        }
        // Where no team plan is sent (0.6.5).
        let objectives_only = PingReach {
            calls: false,
            objectives: true,
        };
        assert_eq!(finish(pit(Slot::TakeHere), objectives_only), None);
    }

    #[test]
    fn a_click_names_that_unit() {
        let t = team(2);
        let enemy = Enemy {
            entity: 25,
            index: None,
        };
        assert_eq!(
            click(Clicked::Enemy(enemy), &t).map(|c| c.order),
            Some(Order::Fight(25))
        );
        assert_eq!(
            click(Clicked::Monster(40, SERPEN), &t).map(|c| c.glyph),
            Some(Glyph::Take)
        );
        assert_eq!(
            click(Clicked::Tower(9, 0), &t).map(|c| c.order),
            Some(Order::Send(
                Send::Objective(Objective::with_lane(4, 0)),
                mask(&[0])
            ))
        );
        assert_eq!(click(Clicked::Ally(3), &t), None, "reserved");
        assert_eq!(
            click(Clicked::Fog((1, 1), Some(0)), &t).map(|c| c.order),
            Some(Order::Fog(Some(0), mask(&[0, 1, 3, 4])))
        );
        let ground = Place::Ground {
            at: (1, 1),
            enemy: None,
        };
        assert_eq!(click(Clicked::Ground(ground), &t), Some(mark(Glyph::Go)));
    }

    #[test]
    fn nothing_reachable_or_nobody_alive_marks_the_place() {
        let mut t = team(0);
        t.reach = PingReach::default();
        for half in Slot::HALVES {
            for s in half {
                let order = wheel(here(lane(2)), s, &t).order;
                assert_eq!(order, Order::Mark, "{s:?}");
            }
        }
        // Dead teammates do not hear.
        let mut t = team(0);
        t.pos = [None; 5];
        t.pos[0] = Some((0, 0));
        let enemy = Enemy {
            entity: 25,
            index: Some(1),
        };
        assert_eq!(
            click(Clicked::Enemy(enemy), &t).map(|c| c.order),
            Some(Order::Mark)
        );
    }
}
