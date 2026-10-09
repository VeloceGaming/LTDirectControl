//! The per-player SDK AI callback: runs on simulation workers for every
//! player of every match. On the viewed match's worker it binds the session,
//! samples HUD/roster/shop data and returns the controlled athlete's input.
use super::*;
use mod_api_stable::{SimOriginV1, StableEntity, StableSim};
use native_timing::MatchKey;

/// Scalars of one `think` call, passed to the per-feature steps below.
#[derive(Clone, Copy)]
struct Call {
    tick: usize,
    player_id: usize,
    athlete: usize,
    lane: Option<usize>,
    side: usize,
}

pub(crate) struct Simulation {
    pub(crate) logger: Arc<Logger>,
    pub(crate) last_sample: Option<(usize, u32, u64, u64, u64)>,
    pub(crate) timing: Arc<native_timing::NativeTiming>,
    pub(crate) movement: Arc<movement::Movement>,
    pub(crate) abilities: Arc<abilities::Abilities>,
    pub(crate) hud: Arc<player_hud::PlayerHud>,
    pub(crate) team: Arc<team_status::TeamStatus>,
    pub(crate) last_manual: bool,
    pub(crate) last_manual_mode: Option<&'static str>,
    pub(crate) session_gate: Arc<RwLock<()>>,
}

impl StablePlayerAi for Simulation {
    fn clone_box(&self) -> Box<dyn StablePlayerAi> {
        Box::new(Self {
            logger: self.logger.clone(),
            last_sample: None,
            timing: self.timing.clone(),
            movement: self.movement.clone(),
            abilities: self.abilities.clone(),
            hud: self.hud.clone(),
            team: self.team.clone(),
            last_manual: false,
            last_manual_mode: None,
            session_gate: self.session_gate.clone(),
        })
    }

    fn id(&self) -> String {
        format!("{MOD_ID}:observer")
    }

    fn matches(&self, init: &StableAiInit) -> bool {
        // Every live player can pace; player zero logs. Only the selected player
        // receive movement; every other callback preserves the native input.
        init.player_id < 10
    }

    fn priority(&self) -> i32 {
        1000
    }

    fn think(&mut self, ctx: &mut StableAiContext<'_>, _base: Option<InputV1>) -> Option<InputV1> {
        // The same session lock, held via its own handle so the steps below
        // may borrow `self` mutably.
        // Stall report markers (crate::worker_watch); Outside again on return.
        let _watch = crate::worker_watch::begin(ctx.player_id());
        let gate = self.session_gate.clone();
        let _session = gate.read().ok()?;
        let tick = ctx.tick();
        let player_id = ctx.player_id();
        let athlete = ctx.athlete_id();
        let lane = ctx.lane().map(|lane| lane.code() as usize);
        let side = ctx.team();
        let call = Call {
            tick,
            player_id,
            athlete,
            lane,
            side,
        };
        let step = |s| crate::worker_watch::enter(s, player_id);
        use crate::worker_watch::Step;
        step(Step::Bind);
        let (
            selected,
            proposal,
            position,
            actor,
            match_key,
            skill_units,
            live_worker,
            attack_in_range,
        ) = {
            let sim = ctx.sim()?;
            let Some(origin) = sim.sim_origin() else {
                if self.last_sample.is_none() {
                    self.logger.write("SIM origin unavailable");
                    self.last_sample = Some((tick, u32::MAX, 0, 0, 0));
                }
                return None;
            };
            let key = (sim.seed(), origin.match_id, origin.set_index);
            if crate::native_timing::left_match(key) {
                return None;
            }
            self.bind_session(&sim, &origin, key, call);
            step(Step::Roster);
            let selected = origin.kind == 2
                && self.timing.owns_worker(key)
                && self.movement.hud_identity() == Some((key, player_id));
            self.sample_prepared_roster(&sim, &origin, key, call);
            step(Step::Team);
            self.sample_team(&sim, &origin, key, call);
            step(Step::Purchases);
            self.trace_purchases(&sim, &origin, key, call);
            step(Step::Shop);
            self.publish_shop(&sim, &origin, key, call);
            step(Step::Hud);
            self.sample_hud(&sim, &origin, key, call);
            step(Step::Controlled);
            let current_champion = selected
                .then(|| {
                    sim.get_player(player_id)
                        .and_then(|player| player.champion())
                })
                .flatten();
            if selected {
                self.observe_controlled(&sim, key, call, &current_champion);
            }
            step(Step::Units);
            let collect_units = selected
                && self.timing.allows_input(key)
                && current_champion.as_ref().is_some_and(|c| c.is_alive());
            let skill_units = if collect_units {
                (0..sim.entity_count())
                    .filter_map(|index| {
                        let unit = sim.entity_at(index)?;
                        (unit.is_alive()
                            && unit.is_targetable()
                            && (unit.team() == side || sim.is_visible(side, unit.id())))
                        .then(|| combat::Unit {
                            id: unit.id(),
                            position: unit.pos(),
                            radius: unit.radius() as u64,
                            is_champion: unit.is_champion(),
                            is_minion: unit.is_minion(),
                            friendly: unit.team() == side,
                            in_cc: (0..unit.cc_count().min(64))
                                .any(|i| unit.cc_at(i).is_some_and(|cc| cc.kind < 10)),
                            is_tower: unit.is_tower() || unit.name().as_deref() == Some("nexus"),
                            body: sprite_picking::entity_body(
                                unit.name().as_deref(),
                                unit.is_champion(),
                                unit.is_minion(),
                                unit.is_tower(),
                                unit.radius() as u64,
                            ),
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            // This callback also runs for other players and background matches.
            // Their empty local list must never erase the live player's allies.
            if let Some(c) = current_champion.as_ref().filter(|_| collect_units) {
                self.movement
                    .observe_hover_units(key, player_id, c.id(), &skill_units);
            }
            if let Some(c) = current_champion.as_ref().filter(|c| c.is_alive()) {
                self.abilities.observe_units(key, c.id(), &skill_units);
            }
            step(Step::AttackRange);
            let attack_ranges = current_champion
                .as_ref()
                .filter(|_| selected && self.timing.allows_input(key))
                .and_then(|champion| native_adapter::attack_ranges(&sim, champion.id()));
            let attack_range = attack_ranges.map(|r| r.0);
            let acquisition_range = current_champion.as_ref().and_then(|champion| {
                let (current, maximum) = attack_ranges?;
                let name = champion.name()?;
                crate::acquisition::observe(&name, current, maximum);
                Some(crate::settings::GLOBAL.get().map_or_else(
                    || {
                        crate::acquisition::radius(
                            &crate::settings::Values::default(),
                            &name,
                            current,
                            maximum,
                        )
                    },
                    |s| s.acquisition_radius(&name, current, maximum),
                ))
            });
            if selected && self.timing.allows_input(key) {
                let sample = current_champion
                    .as_ref()
                    .filter(|c| c.is_alive())
                    .and_then(|c| Some((c.pos(), attack_range?, acquisition_range?)));
                self.movement
                    .observe_acquisition_debug(key, player_id, sample);
            }
            step(Step::Combat);
            let candidate = if selected && self.timing.allows_input(key) {
                current_champion
                    .as_ref()
                    .filter(|champion| champion.is_alive())
                    .and_then(|champion| {
                        let units = (0..sim.entity_count())
                            .filter_map(|index| {
                                let unit = sim.entity_at(index)?;
                                (unit.id() != champion.id()
                                    && unit.team() != side
                                    && unit.is_alive()
                                    && unit.is_targetable()
                                    && sim.is_visible(side, unit.id()))
                                .then(|| combat::Unit {
                                    id: unit.id(),
                                    position: unit.pos(),
                                    radius: unit.radius() as u64,
                                    is_champion: unit.is_champion(),
                                    is_minion: unit.is_minion(),
                                    friendly: false,
                                    in_cc: (0..unit.cc_count().min(64))
                                        .any(|i| unit.cc_at(i).is_some_and(|cc| cc.kind < 10)),
                                    is_tower: unit.is_tower()
                                        || unit.name().as_deref() == Some("nexus"),
                                    body: sprite_picking::entity_body(
                                        unit.name().as_deref(),
                                        unit.is_champion(),
                                        unit.is_minion(),
                                        unit.is_tower(),
                                        unit.radius() as u64,
                                    ),
                                })
                            })
                            .collect();
                        self.movement.combat_input(
                            player_id,
                            champion.pos(),
                            units,
                            acquisition_range,
                        )
                    })
            } else {
                None
            };
            let position = current_champion.as_ref().map(|champion| champion.pos());
            let attack_in_range = current_champion
                .as_ref()
                .zip(candidate.as_ref())
                .filter(|(_, (input, _))| input.kind == mod_api_stable::InputKindV1::Attack.code())
                .and_then(|(champion, (_, chase))| {
                    let range = attack_range?;
                    Some(combat::attack_in_range(champion.pos(), (*chase)?, range))
                });
            self.log_sample(&sim, &origin, call);
            let actor = current_champion
                .as_ref()
                .filter(|c| c.is_alive())
                .map(|c| c.id());
            (
                selected,
                candidate,
                position,
                actor,
                key,
                skill_units,
                origin.kind == 2 && self.timing.owns_worker(key),
                attack_in_range,
            )
        };
        step(Step::Items);
        if live_worker {
            self.hud.capture_items(ctx, match_key, &self.logger);
        }
        step(Step::Abilities);
        let skill = actor
            .zip(position)
            .filter(|_| selected && self.timing.allows_input(match_key))
            .and_then(|(actor, position)| {
                self.abilities.take_input(
                    match_key,
                    actor,
                    position,
                    &skill_units,
                    |input| ctx.is_valid_input(input),
                    &self.logger,
                )
            });
        // Keep the attack order while waiting in range. Outside range (or if
        // the guarded range read is unavailable), preserve native approach.
        let candidate = skill.or_else(|| {
            proposal.map(|(request, chase)| {
                if request.kind == mod_api_stable::InputKindV1::Return.code()
                    && !ctx.is_valid_input(&request)
                {
                    self.movement.recall_rejected(&self.logger);
                    let p = position.unwrap_or_default();
                    InputV1::move_to(p.0, p.1)
                } else if request.kind == mod_api_stable::InputKindV1::Attack.code()
                    && !ctx.is_valid_input(&request)
                {
                    combat::rejected_attack(position.unwrap_or_default(), chase, attack_in_range)
                } else {
                    request
                }
            })
        });
        step(Step::Dispatch);
        let valid = candidate
            .as_ref()
            .is_some_and(|input| ctx.is_valid_input(input));
        let mode = candidate.map(|input| {
            if input.kind == mod_api_stable::InputKindV1::Attack.code() {
                "attack"
            } else if input.kind == mod_api_stable::InputKindV1::Return.code() {
                "recall"
            } else if input.kind >= mod_api_stable::InputKindV1::Skill.code() {
                "skill"
            } else if position == Some((input.x, input.y)) {
                "hold"
            } else {
                "move"
            }
        });
        if selected
            && (candidate.is_some()
                && (tick.is_multiple_of(60)
                    || valid != self.last_manual
                    || mode != self.last_manual_mode)
                || candidate.is_none() && self.last_manual)
        {
            self.logger.write(&format!("MANUAL input tick={tick} player={player_id} mode={mode:?} pos={position:?} request={candidate:?} valid={valid} attack_in_range={attack_in_range:?} base={_base:?}"));
        }
        if valid && live_worker {
            let stamp = skill
                .and_then(|input| self.abilities.dispatched_stamp(input.kind))
                .or_else(|| {
                    skill
                        .is_none()
                        .then(|| self.movement.take_command_stamp())
                        .flatten()
                });
            if let (Some(stamp), Some(input)) = (stamp, candidate.as_ref()) {
                self.timing
                    .trace_dispatch(match_key, stamp, input, &self.logger);
            }
        }
        self.last_manual = valid;
        self.last_manual_mode = mode;
        if selected {
            native_adapter::arm_stop(
                match_key,
                actor.filter(|_| valid && matches!(mode, Some("hold" | "move" | "attack"))),
                valid && mode == Some("hold"),
                valid && self.movement.take_cancel_recall(player_id),
                candidate
                    .filter(|_| valid && mode == Some("attack") && attack_in_range == Some(true))
                    .map(|input| input.target.target_id),
            );
        }
        if candidate.is_some() && !valid {
            // Never silently Pass a rejected manual tick while the overlay
            // still claims control. Release the session and report the reason.
            self.timing
                .cancel("Manual command rejected; native AI restored", &self.logger);
        }
        // An idle living champion now returns Some(move_to(current_position)).
        // None keeps native input only outside control, with no living actor,
        // or after an explicitly logged failure releases the coordinator.
        candidate.filter(|_| valid)
    }
}

impl Simulation {
    /// Bind this worker to the session and register the player at tick 1.
    fn bind_session(&self, sim: &StableSim<'_>, origin: &SimOriginV1, key: MatchKey, call: Call) {
        let Call {
            tick,
            player_id,
            athlete,
            lane,
            side,
        } = call;
        if origin.kind == 2 {
            // Bind before touching selection; late result/re-simulation
            // callbacks cannot register players for another session.
            self.timing.observe(key, tick, &self.logger);
            if tick == 1 && self.timing.owns_worker(key) {
                self.movement.begin_match(key, &self.logger);
                if let Some(lane) = lane {
                    let champion = sim
                        .get_player(player_id)
                        .and_then(|player| player.champion())
                        .and_then(|champion| champion.name())
                        .unwrap_or_default();
                    self.movement.register_player(
                        key,
                        own_selection::PlayerIdentity {
                            player: player_id,
                            athlete,
                            lane,
                            side,
                            champion,
                        },
                        &self.logger,
                    );
                }
            }
            // Identify this worker only. The SDK callback NEVER waits.
        }
    }
    /// Tick 1 of the viewed match: every player's HUD data before Start.
    fn sample_prepared_roster(
        &self,
        sim: &StableSim<'_>,
        origin: &SimOriginV1,
        key: MatchKey,
        call: Call,
    ) {
        let Call { tick, .. } = call;
        // Startup tick 1 precedes presentation readiness. Capture roster
        // HUD data here rather than hiding it behind accepts_sample(),
        // whose battlefield gate only opens after startup is held.
        if origin.kind == 2 && tick == 1 && self.timing.owns_worker(key) {
            for index in 0..sim.player_count() {
                if let Some(p) = sim.player_at(index) {
                    let c = p.champion();
                    self.hud.observe_prepared(player_hud::Snapshot {
                        key,
                        player: p.id(),
                        champion: c.as_ref().and_then(|c| c.name()).unwrap_or_default(),
                        level: p.level(),
                        hp: c.as_ref().map(|c| c.hp()),
                        alive: p.is_alive(),
                        respawn: p.respawn_time(),
                        gold: p.gold(),
                        kda: (p.kills(), p.deaths(), p.assists()),
                        cs: p.cs(),
                        cooldowns: p.cooldowns().map_or([0; 3], |(_, q, w, r)| [q, w, r]),
                        items: p.item_keys(),
                        build: self.hud.build(sim, p.id()),
                    });
                }
            }
            // 0.70 diagnostic: which actor id is which champion, for the
            // PREVIEW TREE inventory lines of this match.
            let actors = (0..sim.player_count())
                .filter_map(|i| {
                    let c = sim.player_at(i)?.champion()?;
                    Some(format!("{}={}", c.id(), c.name().unwrap_or_default()))
                })
                .collect::<Vec<_>>();
            // Once per match: every player's think runs this tick.
            static LAST: std::sync::Mutex<Option<MatchKey>> = std::sync::Mutex::new(None);
            if LAST
                .lock()
                .is_ok_and(|mut last| last.replace(key) != Some(key))
            {
                self.logger.write(&format!(
                    "PREVIEW ROSTER key={key:?} actors=[{}]",
                    actors.join(", ")
                ));
            }
        }
    }
    /// Tab scoreboard data for all ten players.
    fn sample_team(&self, sim: &StableSim<'_>, origin: &SimOriginV1, key: MatchKey, call: Call) {
        let Call { tick, .. } = call;
        // Any living actor can read all ten players, including dead ones.
        // Copy only SDK scalars on this session's original live worker.
        if origin.kind == 2 && self.timing.accepts_sample(key) && self.team.needs_sample(key, tick)
        {
            let players = (0..sim.player_count())
                .filter_map(|index| {
                    let p = sim.player_at(index)?;
                    Some(team_status::Player {
                        id: p.id(),
                        side: p.team(),
                        lane: p.lane()?.code() as usize,
                        champion: p.champion().and_then(|c| c.name()).unwrap_or_default(),
                        alive: Some(p.is_alive()),
                        respawn: p.respawn_time(),
                        level: p.level(),
                        gold: p.gold(),
                        kda: (p.kills(), p.deaths(), p.assists()),
                        cs: p.cs(),
                        items: p.item_keys(),
                        build: self.hud.build(sim, p.id()),
                        fresh: true,
                    })
                })
                .collect();
            self.team.observe(key, players, &self.logger);
        }
    }
    /// Diagnostic purchase records for all ten players.
    fn trace_purchases(
        &self,
        sim: &StableSim<'_>,
        origin: &SimOriginV1,
        key: MatchKey,
        call: Call,
    ) {
        let Call { tick, .. } = call;
        // Read-only shop investigation: every purchase step of all ten
        // players, plus a once-per-match check of the native buyer code.
        if origin.kind == 2
            && self.timing.accepts_sample(key)
            && shop_trace::SHOP_TRACE.due(key, tick)
        {
            if shop_trace::SHOP_TRACE.take_anchor_check() {
                for line in native_adapter::buyer_anchor_report() {
                    self.logger.write(&line);
                }
            }
            let players = (0..sim.player_count())
                .filter_map(|index| {
                    let p = sim.player_at(index)?;
                    Some((p.id(), p.team(), p.gold()))
                })
                .collect::<Vec<_>>();
            shop_trace::SHOP_TRACE.observe(
                tick,
                &players,
                |id| {
                    sim.get_player(id)
                        .map(|p| p.item_keys())
                        .unwrap_or_default()
                },
                |id| shop_trace::Detail {
                    champion: sim
                        .get_player(id)
                        .and_then(|p| p.champion())
                        .and_then(|c| c.name())
                        .unwrap_or_default(),
                    build: self.hud.build(sim, id),
                    native_owned: native_adapter::player_owned_len(sim, id),
                },
                |line| self.logger.write(line),
            );
        }
    }
    /// Manual shopping: publish the controlled champion's shop state.
    fn publish_shop(&self, sim: &StableSim<'_>, origin: &SimOriginV1, key: MatchKey, call: Call) {
        let Call { tick, .. } = call;
        // Manual shopping: once per tick, from whichever actor runs first,
        // publish the controlled champion's shop data and next purchase
        // step. Once the champion's native object is known with Manual
        // shopping on, missing item data fails closed (buys nothing).
        // Owning the worker suffices: starting items are bought in the
        // first ticks, before the battlefield view (accepts_sample) opens.
        if origin.kind == 2
            && (self.timing.accepts_sample(key) || self.timing.owns_worker(key))
            && shop::SHOP.due(key, tick)
        {
            let controlled = self
                .movement
                .hud_identity()
                .filter(|(hud_key, _)| *hud_key == key)
                .map(|(_, player)| player);
            let keys = self.hud.catalog_keys();
            let cat = shop::SHOP.catalogue_for(key, || {
                let meta = self.hud.item_metadata(key)?;
                shop::catalogue(&keys, meta.as_ref())
            });
            let live = controlled.and_then(|id| {
                let player = sim.get_player(id)?;
                let index = |k: &String| keys.iter().position(|x| x == k);
                Some(shop::Live {
                    owned: player
                        .item_keys()
                        .iter()
                        .map(index)
                        .collect::<Option<Vec<_>>>()?,
                    build: native_adapter::player_build(sim, id)?,
                    gold: player.gold(),
                    capacity: native_adapter::item_slot_capacity().unwrap_or(0),
                })
            });
            let mode = if settings::option("manual_shop") != 1. {
                shop::Mode::Native("Manual shopping off")
            } else if !native_adapter::shop_ready() {
                shop::Mode::Native("buyer hooks unavailable")
            } else if !matches!(
                self.timing.worker_phase(key),
                Some(
                    native_timing::Phase::Loading
                        | native_timing::Phase::Ready
                        | native_timing::Phase::Running
                        | native_timing::Phase::Paused
                )
            ) {
                // From champion lock (including loading and the
                // Start-control wait, when the game buys starting items)
                // until control returns to the AI (F12).
                shop::Mode::Native("AI control")
            } else if matches!(
                self.timing.worker_phase(key),
                Some(native_timing::Phase::Loading | native_timing::Phase::Ready)
            ) {
                // Starting items are bought at tick 1, while the lane may
                // still change: hold every champion of this match (by its
                // native object, so other simulations are untouched)
                // until Start; the others then buy on the next tick.
                shop::Mode::Prestart {
                    controlled: controlled.and_then(|id| native_adapter::native_player(sim, id)),
                    hold: (0..sim.player_count())
                        .filter_map(|i| native_adapter::native_player(sim, sim.player_at(i)?.id()))
                        .collect(),
                }
            } else {
                match controlled.and_then(|id| native_adapter::native_player(sim, id)) {
                    Some(pointer) => shop::Mode::Manual(pointer),
                    None => shop::Mode::Native("controlled champion object unknown"),
                }
            };
            shop::SHOP.set_vanilla_order(settings::option("shop_vanilla_order") == 1.);
            shop::SHOP.publish(mode, cat, live, |line| self.logger.write(line));
        }
    }
    /// HUD strip data for the controlled player.
    fn sample_hud(&self, sim: &StableSim<'_>, origin: &SimOriginV1, key: MatchKey, call: Call) {
        let Call { tick, .. } = call;
        // Other living actors still run think while the selected champion
        // is dead. Read the selected player from this same worker's sim.
        if let Some((hud_key, hud_player)) = self.movement.hud_identity().filter(|(hud_key, _)| {
            origin.kind == 2 && *hud_key == key && self.timing.accepts_sample(key)
        }) {
            if self.hud.needs_sample(hud_key, hud_player, tick) {
                if let Some(player) = sim.get_player(hud_player) {
                    let champion = player.champion();
                    let cooldowns = player.cooldowns().map_or([0; 3], |(_, q, w, r)| [q, w, r]);
                    if !player.is_alive() {
                        self.movement.observe_position(None);
                        self.abilities.observe_actor(key, None, None, cooldowns);
                    }
                    crate::worker_watch::enter(
                        crate::worker_watch::Step::StatsPanel,
                        call.player_id,
                    );
                    crate::stats_panel::sample(sim, champion.as_ref(), &self.logger);
                    crate::worker_watch::enter(crate::worker_watch::Step::Hud, call.player_id);
                    self.hud.observe_tick(
                        player_hud::Snapshot {
                            key,
                            player: hud_player,
                            champion: champion.as_ref().and_then(|c| c.name()).unwrap_or_default(),
                            level: player.level(),
                            hp: champion.as_ref().map(|c| c.hp()),
                            alive: player.is_alive(),
                            respawn: player.respawn_time(),
                            gold: player.gold(),
                            kda: (player.kills(), player.deaths(), player.assists()),
                            cs: player.cs(),
                            cooldowns,
                            items: player.item_keys(),
                            build: self.hud.build(sim, hud_player),
                        },
                        tick,
                        &self.logger,
                    );
                }
            }
        }
    }
    /// Position, abilities, level and champion of the controlled athlete.
    fn observe_controlled(
        &self,
        sim: &StableSim<'_>,
        key: MatchKey,
        call: Call,
        current_champion: &Option<StableEntity<'_, '_>>,
    ) {
        let Call { player_id, .. } = call;
        self.movement.observe_position(
            current_champion
                .as_ref()
                .filter(|c| c.is_alive())
                .map(|c| c.pos()),
        );
        let living = current_champion.as_ref().filter(|c| c.is_alive());
        let cooldowns = sim
            .get_player(player_id)
            .and_then(|p| p.cooldowns())
            .map_or([0; 3], |(_, q, w, r)| [q, w, r]);
        self.abilities.observe_actor(
            key,
            living.map(|c| c.id()),
            living.map(|c| c.pos()),
            cooldowns,
        );
        if let (Some(c), Some(p)) = (living, sim.get_player(player_id)) {
            self.abilities.observe_level(key, c.id(), p.level());
            self.abilities.observe_champion(key, c.id(), c.name());
        }
    }
    /// Periodic SIM/ROSTER diagnostic lines (player 0 only).
    fn log_sample(&mut self, sim: &StableSim<'_>, origin: &SimOriginV1, call: Call) {
        let Call {
            tick,
            player_id,
            athlete,
            ..
        } = call;
        // Record each second of foreground startup, then every ten seconds.
        let startup_sample = origin.kind == 2 && tick <= 1200 && tick.is_multiple_of(60);
        let sample = (
            tick,
            origin.kind,
            origin.match_id,
            origin.replay_id,
            origin.set_index,
        );
        if player_id == 0
            && (tick <= 2 || startup_sample || tick.is_multiple_of(600))
            && self.last_sample != Some(sample)
        {
            self.last_sample = Some(sample);
            self.logger.write_sample(
        &format!(
            "SIM tick={tick} seed={} kind={} match={} replay={} set={} athlete={} players={} thread={:?}",
            sim.seed(),
            origin.kind,
            origin.match_id,
            origin.replay_id,
            origin.set_index,
            athlete,
            sim.player_count(), std::thread::current().id()
        ),
        origin.kind <= 1,
    );
            // Foreground MatchView only; do not fill the log with background rosters.
            if origin.kind == 2 && tick <= 2 {
                native_adapter::capture_trace(
                    &format!("SDK foreground AI tick {tick}"),
                    &self.logger,
                );
                for index in 0..sim.player_count() {
                    let Some(player) = sim.player_at(index) else {
                        continue;
                    };
                    let champ = player
                        .champion()
                        .map(|entity| (entity.name(), entity.pos(), entity.hp()));
                    self.logger.write(&format!(
                        "ROSTER player={} team={} lane={:?} champion={champ:?} cooldowns={:?}",
                        player.id(),
                        player.team(),
                        player.lane(),
                        player.cooldowns()
                    ));
                }
            }
        }
    }
}
