//! Players Info, for Horizon: what the client knows of the player you look at (health, game
//! mode, ping, equipment and its enchantments, posture and what they're doing, what they ride,
//! where they are, what just happened to them), drawn around them on screen. A trait mod: the
//! launcher links this crate into the game and calls `hooks()`.
//!
//! It shows what the server sends the client and nothing more, and only for a player in plain
//! view: line of sight is checked, walls hide them.

mod anim;
mod around;
mod card;
mod config;
mod events;
mod gear;
mod health;
mod i18n;
mod settings;
mod sync;
mod target;

use around::Anchors;
use card::color::{GOLD, GRAY, RED, WHITE, YELLOW};
use card::{EventLine, HeartStyle, Segments, Slot, Span, View};
use config::{Config, Layout};
use events::{Feeds, Kind as EventKind};
use health::Tracker;
use horizon_client::mod_api::{
    ClientDisconnectingEvent, ClientLevelUnloadEvent, ClientPacketEvent, ClientboundPlayPacket,
    ConfigLoadingEvent, ConfigReloadingEvent, EntityRef, EquipmentSlot, Hooks, Host, HudEvent,
    ItemStack, KeyEvent, ModsEvent, NativeCamera, NativeClientEntity, NativeConfig,
    NativeConfigKind, NativeMod, RegisterConfigsEvent, SettingsClickEvent, SettingsEvent, Text,
};
use horizon_protocol::entity_data::Value;
use i18n::Lang;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use sync::{Posture, Sync};

const MOD_ID: &str = "players_info";
const TITLE: &str = concat!("Players Info ", env!("CARGO_PKG_VERSION"));
const KEY_TOGGLE: (&str, &str, &str) = ("toggle", "Toggle Players Info", "key.keyboard.i");
const PLAYER: &str = "minecraft:player";

/// A name tag's line, in blocks: 9 font pixels, 1.15 apart, 0.025 blocks each
/// (`EntityRenderer.extractNameTags`).
const TAG_LINE: f64 = 9.0 * 1.15 * 0.025;
/// A name tag's font pixel, in blocks, at scale 1.
const FONT_PIXEL: f64 = 0.025;
/// `Scoreboard.DISPLAY_SLOT_BELOW_NAME`: its score line shows under names within 10 blocks.
const BELOW_NAME_SLOT: i32 = 2;
const BELOW_NAME_DISTANCE: f64 = 10.0;
/// What the card shows, in order: hands, then armor head to feet.
const CARD_SLOTS: [(Slot, EquipmentSlot); 6] = [
    (Slot::Main, EquipmentSlot::MainHand),
    (Slot::Off, EquipmentSlot::OffHand),
    (Slot::Armor(0), EquipmentSlot::Head),
    (Slot::Armor(1), EquipmentSlot::Chest),
    (Slot::Armor(2), EquipmentSlot::Legs),
    (Slot::Armor(3), EquipmentSlot::Feet),
];
/// `GameType.SPECTATOR`.
const SPECTATOR: i32 = 3;
/// Seconds after an entity comes into range during which what the server tells about it
/// (its seat, its game mode) is its state, not news.
const SETTLING: f64 = 0.75;
/// Events kept on the card.
const CARD_EVENTS: usize = 3;

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(PlayersInfo::new(Clock::Real(Instant::now())))
}

/// `hooks()` with a clock the caller moves, in seconds: for tests.
#[doc(hidden)]
pub fn hooks_with_clock(clock: Arc<Mutex<f64>>) -> Box<dyn Hooks> {
    Box::new(PlayersInfo::new(Clock::Manual(clock)))
}

enum Clock {
    Real(Instant),
    Manual(Arc<Mutex<f64>>),
}

impl Clock {
    fn now(&self) -> f64 {
        match self {
            Clock::Real(start) => start.elapsed().as_secs_f64(),
            Clock::Manual(t) => *t.lock().unwrap_or_else(PoisonError::into_inner),
        }
    }
}

struct PlayersInfo {
    clock: Clock,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    config: Config,
    config_path: Option<PathBuf>,
    /// The text last read or written, so the mod's own saves don't count as edits.
    config_text: Option<String>,
    health: Tracker,
    sync: Sync,
    feeds: Feeds,
    /// The local player's entity id, from the login packet: never shown.
    own_entity: Option<u32>,
    /// Whether a scoreboard objective shows under names (it lifts the name a line).
    below_name: bool,
    /// Entity id to its type id, asked once per entity.
    kinds: HashMap<u32, String>,
    /// When each entity came into range.
    spawned: HashMap<u32, f64>,
    /// When each entity last swung its main arm.
    swings: HashMap<u32, f64>,
    /// The level the client is in (`minecraft:overworld`), from the login and respawn packets.
    level: Option<String>,
    /// A hardcore world: the card draws hardcore hearts, as the health bar does.
    hardcore: bool,
    /// Players' entity ids by UUID, as of the last HUD frame.
    players: HashMap<u128, u32>,
    /// Skin textures for faces, by UUID, and when they were last asked for.
    skins: HashMap<u128, String>,
    skins_at: Option<f64>,
    card: CardState,
    last_frame: Option<f64>,
}

#[derive(Default)]
struct CardState {
    anim: card::Anim,
    /// The player the card should be about, and the one it shows (they differ while it
    /// changes player).
    wanted: Option<u32>,
    shown: Option<u32>,
    last_seen: f64,
    /// The last frame's view, to fade out with when the player is gone.
    view: Option<View>,
    /// The shown player's main-hand item, to notice a switch.
    main_hand: Option<(u32, String)>,
    /// Where the shown player last was on screen, for the layout around them.
    anchors: Option<Anchors>,
}

impl PlayersInfo {
    fn new(clock: Clock) -> Self {
        Self {
            clock,
            state: Mutex::new(State::new()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    fn new() -> Self {
        Self {
            config: Config::initial(),
            ..Self::default()
        }
    }

    fn lang(&self) -> Lang {
        Lang::of(self.config.language)
    }

    fn load(&mut self, text: &str, path: PathBuf) -> Result<(), String> {
        self.config_path = Some(path);
        if self.config_text.as_deref() == Some(text) {
            return Ok(());
        }
        let config = Config::parse(text)?;
        self.config = config;
        self.config_text = Some(text.to_owned());
        Ok(())
    }

    fn save(&mut self) {
        let text = self.config.to_toml();
        if self.config_text.as_deref() == Some(text.as_str()) {
            return;
        }
        let Some(path) = &self.config_path else {
            log::warn!("Players Info: the config file's place is unknown; not saved");
            return;
        };
        match std::fs::write(path, &text) {
            Ok(()) => self.config_text = Some(text),
            Err(e) => log::warn!("Players Info: could not save {}: {e}", path.display()),
        }
    }

    /// The client's world went: every entity with it.
    fn reset_world(&mut self) {
        self.health.clear();
        self.sync.clear_world();
        self.feeds.clear();
        self.kinds.clear();
        self.spawned.clear();
        self.swings.clear();
        self.players.clear();
        self.card = CardState::default();
    }

    fn forget_entity(&mut self, id: u32) {
        self.health.remove(id);
        self.sync.remove_entity(id);
        self.feeds.remove(id);
        self.kinds.remove(&id);
        self.spawned.remove(&id);
        self.swings.remove(&id);
    }

    fn kind(&mut self, host: &dyn Host, id: u32) -> String {
        if let Some(kind) = self.kinds.get(&id) {
            return kind.clone();
        }
        let kind = host
            .entity(id)
            .and_then(|e| host.entity_kind(e))
            .unwrap_or_default();
        // Not known yet (a packet ahead of the entity): asked again next time.
        if !kind.is_empty() {
            self.kinds.insert(id, kind.clone());
        }
        kind
    }

    fn is_other_player(&mut self, host: &dyn Host, id: u32) -> bool {
        Some(id) != self.own_entity && self.kind(host, id) == PLAYER
    }

    fn settling(&self, id: u32, now: f64) -> bool {
        self.spawned.get(&id).is_some_and(|t| now - t < SETTLING)
    }

    /// Adds an event to a player's feed (other entities have none).
    fn event(&mut self, host: &dyn Host, id: u32, kind: EventKind, now: f64) {
        if !self.is_other_player(host, id) {
            return;
        }
        self.feeds.push(id, kind, now);
    }

    fn entity_data(&mut self, id: i32, values: &[(u8, i32, Value)]) {
        let Ok(id) = u32::try_from(id) else {
            return;
        };
        for (index, _, value) in values {
            match (*index, value) {
                (health::DATA_HEALTH, Value::Float(h)) => self.health.set_health(id, *h),
                (health::DATA_PLAYER_ABSORPTION, Value::Float(a)) => {
                    self.health.set_absorption(id, *a);
                }
                (sync::DATA_FLAGS, Value::Byte(flags)) => self.sync.set_shared(id, *flags),
                (sync::DATA_POSE, Value::Id(pose)) => self.sync.set_pose(id, *pose),
                (sync::DATA_LIVING_FLAGS, Value::Byte(flags)) => {
                    self.sync.set_living(id, *flags);
                }
                (sync::DATA_TICKS_FROZEN, Value::Int(ticks)) => self.sync.set_frozen(id, *ticks),
                _ => {}
            }
        }
    }

    fn packet(&mut self, host: &dyn Host, packet: &ClientboundPlayPacket, now: f64) {
        let id = |raw: i32| u32::try_from(raw).ok();
        match packet {
            ClientboundPlayPacket::Login(login) => {
                self.reset_world();
                self.level = Some(login.spawn.dimension.clone());
                self.sync.clear();
                self.skins.clear();
                self.skins_at = None;
                self.own_entity = id(login.player_id);
                self.hardcore = login.hardcore;
                self.below_name = false;
            }
            // `handleRespawn` keeps the level, and every entity in it, when the player respawns
            // in the same one (after a death, or a plugin's respawn to refresh a skin): the
            // server sends nothing again of the players around, so what is known of them stays.
            ClientboundPlayPacket::Respawn(respawn) => {
                if self.level.as_deref() != Some(respawn.spawn.dimension.as_str()) {
                    self.reset_world();
                }
                self.level = Some(respawn.spawn.dimension.clone());
            }
            ClientboundPlayPacket::AddEntity(add) => {
                if let Some(e) = id(add.id) {
                    // Ids are reused: nothing of the last entity with this one carries over.
                    self.forget_entity(e);
                    self.spawned.insert(e, now);
                }
            }
            ClientboundPlayPacket::SetEntityData(data) => self.entity_data(data.id, &data.values),
            ClientboundPlayPacket::RemoveEntities(removed) => {
                for e in removed.ids.iter().filter_map(|&raw| id(raw)) {
                    self.forget_entity(e);
                }
            }
            ClientboundPlayPacket::SetDisplayObjective(display)
                if display.slot == BELOW_NAME_SLOT =>
            {
                self.below_name = !display.objective.is_empty();
            }
            ClientboundPlayPacket::PlayerInfoUpdate(update) => {
                for entry in &update.entries {
                    let changed =
                        self.sync
                            .update_listing(entry.uuid, entry.game_mode, entry.latency);
                    if let (Some(mode), Some(&e)) = (changed, self.players.get(&entry.uuid))
                        && !self.settling(e, now)
                    {
                        self.event(host, e, EventKind::GameMode(mode), now);
                    }
                }
            }
            ClientboundPlayPacket::PlayerInfoRemove(removed) => {
                for uuid in &removed.uuids {
                    self.sync.remove_listing(*uuid);
                    self.skins.remove(uuid);
                }
            }
            ClientboundPlayPacket::EntityEvent(event) => {
                if let (Some(e), Some(kind)) =
                    (id(event.id), events::from_entity_event(event.event))
                {
                    self.event(host, e, kind, now);
                }
            }
            ClientboundPlayPacket::Animate(animate) => {
                let Some(e) = id(animate.id) else {
                    return;
                };
                if animate.action == 0 {
                    self.swings.insert(e, now);
                }
                if let Some(kind) = events::from_animation(animate.action) {
                    self.event(host, e, kind, now);
                }
            }
            // 26.x's swing with a kind and a duration (a spear's stab, a mace's whack).
            ClientboundPlayPacket::SwingAnimation(swing) if swing.hand == 0 => {
                if let Some(e) = id(swing.id) {
                    self.swings.insert(e, now);
                }
            }
            ClientboundPlayPacket::HurtAnimation(hurt) => {
                if let Some(e) = id(hurt.id) {
                    self.event(host, e, EventKind::Hurt { by: None }, now);
                }
            }
            ClientboundPlayPacket::DamageEvent(damage) => {
                let Some(e) = id(damage.id) else {
                    return;
                };
                let by = damage
                    .cause
                    .and_then(id)
                    .filter(|&cause| cause != e)
                    .and_then(|cause| name_of(self, host, cause));
                self.event(host, e, EventKind::Hurt { by }, now);
            }
            ClientboundPlayPacket::SetPassengers(passengers) => {
                let Some(vehicle) = id(passengers.vehicle) else {
                    return;
                };
                let riders: Vec<u32> = passengers
                    .passengers
                    .iter()
                    .filter_map(|&r| id(r))
                    .collect();
                let boarding = self.sync.set_passengers(vehicle, &riders);
                if boarding.on.is_empty() && boarding.off.is_empty() {
                    return;
                }
                let what = i18n::entity_name(&self.kind(host, vehicle));
                let news =
                    |s: &Self, rider: u32| !s.settling(rider, now) && !s.settling(vehicle, now);
                for rider in boarding.on {
                    if news(self, rider) {
                        self.event(host, rider, EventKind::Mounted(what.clone()), now);
                    }
                }
                for rider in boarding.off {
                    if news(self, rider) {
                        self.event(host, rider, EventKind::Dismounted(what.clone()), now);
                    }
                }
            }
            _ => {}
        }
    }
}

/// An entity's name: a player's, a custom name, else its type's in the game's language.
fn name_of(s: &mut State, host: &dyn Host, id: u32) -> Option<String> {
    let entity = host.entity(id)?;
    let kind = s.kind(host, id);
    if kind == PLAYER {
        return host.entity_name(entity);
    }
    Some(i18n::entity_name(&kind)).filter(|n| !n.is_empty())
}

impl Hooks for PlayersInfo {
    fn mods(&self, _host: &mut dyn Host, event: &mut ModsEvent<'_>) {
        let (name, label, default) = KEY_TOGGLE;
        let lang = self.state().lang();
        let description = lang.pick(
            "Les infos du joueur que tu regardes, autour de lui : santé, mode de jeu, ping, \
             équipement, posture, monture, position, événements. Réglages fins : \
             config/players_info-client.toml.",
            "What you can know of the player you look at, around them: health, game mode, \
             ping, equipment, posture, mount, position, events. More settings: \
             config/players_info-client.toml.",
        );
        event.mods.push(NativeMod {
            id: MOD_ID.into(),
            title: TITLE.into(),
            description: description.into(),
            keys: vec![[name.into(), label.into(), default.into()]],
        });
    }

    fn register_configs(&self, _host: &mut dyn Host, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(NativeConfig::new(
            MOD_ID,
            NativeConfigKind::Client,
            &Config::initial().to_toml(),
        ));
    }

    fn config_loading(&self, host: &mut dyn Host, event: &mut ConfigLoadingEvent<'_>) {
        if event.config.mod_id == MOD_ID {
            self.read_config(host, &event.config.text, event.config.path.clone());
        }
    }

    fn config_reloading(&self, host: &mut dyn Host, event: &mut ConfigReloadingEvent<'_>) {
        if event.config.mod_id == MOD_ID {
            self.read_config(host, &event.config.text, event.config.path.clone());
        }
    }

    fn settings(&self, _host: &mut dyn Host, event: &mut SettingsEvent<'_>) {
        if event.mod_id == MOD_ID {
            event.buttons.extend(settings::labels(&self.state().config));
        }
    }

    fn settings_click(&self, _host: &mut dyn Host, event: &mut SettingsClickEvent<'_>) {
        if event.mod_id != MOD_ID {
            return;
        }
        let mut s = self.state();
        if settings::click(&mut s.config, event.button) {
            s.save();
        }
    }

    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        let (name, _, default) = KEY_TOGGLE;
        if !event.pressed || !host.playing() || host.mod_key(MOD_ID, name, default) != event.key {
            return;
        }
        let mut s = self.state();
        s.config.enabled ^= true;
        let lang = s.lang();
        let message = if s.config.enabled {
            lang.pick("Players Info activé", "Players Info on")
        } else {
            lang.pick("Players Info désactivé", "Players Info off")
        };
        s.save();
        drop(s);
        host.client_action_bar(Text::literal(message), false);
    }

    fn client_packet(&self, host: &mut dyn Host, event: &mut ClientPacketEvent<'_>) {
        let now = self.clock.now();
        self.state().packet(host, event.packet, now);
    }

    fn client_level_unload(&self, _host: &mut dyn Host, _event: &mut ClientLevelUnloadEvent<'_>) {
        // Only the card goes. What the server told of the entities stays: the game can drop
        // the old level after the new one's first entity packets, and the login and respawn
        // packets, which come before those, already start over.
        self.state().card = CardState::default();
    }

    fn client_disconnecting(&self, _host: &dyn Host, _event: &mut ClientDisconnectingEvent) {
        let mut s = self.state();
        s.reset_world();
        s.sync.clear();
        s.skins.clear();
        s.skins_at = None;
        s.own_entity = None;
        s.level = None;
        s.below_name = false;
    }

    fn hud(&self, host: &mut dyn Host, event: &mut HudEvent<'_>) {
        let now = self.clock.now();
        let mut guard = self.state();
        let s = &mut *guard;
        let dt = s.last_frame.map_or(0.0, |t| now - t).clamp(0.0, 0.1) as f32;
        s.last_frame = Some(now);
        let Some(camera) = host.camera() else {
            return;
        };
        let partial = host
            .client_state()
            .map_or(f64::from(camera.partial_tick), |c| c.world_partial_tick)
            .clamp(0.0, 1.0);
        let entities = host.client_entities();
        s.players = entities
            .iter()
            .filter_map(|e| Some((e.player_uuid?, e.id)))
            .collect();
        s.feeds.prune(now);

        let on = s.config.enabled && s.config.card.enabled;
        let picked = if on {
            pick(s, host, &camera, &entities, partial)
        } else {
            None
        };
        follow(s, &entities, picked, now, dt, on);
        if s.card.anim.presence <= 0.005 && s.card.wanted.is_none() {
            return;
        }

        let screen = [event.batch.width, event.batch.height];
        let layout = s.config.card.layout;
        if let Some(id) = s.card.shown {
            match entities.iter().find(|e| e.id == id) {
                Some(entity) => {
                    refresh_skins(s, host, entity.player_uuid, now);
                    let view = build_view(s, host, &camera, entity, partial, now);
                    if layout == Layout::Around {
                        match anchors(s, host, &camera, entity, &view.name, partial, screen) {
                            Some(at) => s.card.anchors = Some(at),
                            // Off screen: the pieces fade out where they last were.
                            None => s.card.wanted = None,
                        }
                    }
                    s.card.view = Some(view);
                }
                // Gone (out of range, logged off): the card fades out as it last was.
                None => s.card.wanted = None,
            }
        }
        let Some(view) = s.card.view.take() else {
            return;
        };
        let style = card::Style {
            placement: s.config.card.placement,
            offset: [s.config.card.offset_x as f32, s.config.card.offset_y as f32],
            scale: s.config.card.scale,
            opacity: s.config.card.opacity,
        };
        let lang = s.lang();
        let measure = |text: &str| host.text_width(text);
        let painted = match (layout, s.card.anchors) {
            (Layout::Around, Some(at)) => around::paint(
                &view,
                &mut s.card.anim,
                &at,
                lang,
                screen,
                s.config.card.scale,
                now,
                dt,
                &measure,
            ),
            (Layout::Around, None) => {
                s.card.view = Some(view);
                return;
            }
            (Layout::Panel, _) => card::paint(
                &view,
                &mut s.card.anim,
                &style,
                lang,
                screen,
                now,
                dt,
                &measure,
            ),
        };
        for (sprite, rect, color) in &painted.boxes {
            host.gui_nine_slice(event.batch, sprite, *rect, *color);
        }
        host.gui_paint(event.batch, &painted.draws);
        for (i, at, size) in painted.items {
            if let Some((_, stack)) = view.items.get(i) {
                host.item_stack(event.batch, stack, at, size);
            }
        }
        s.card.view = Some(view);
    }
}

impl PlayersInfo {
    fn read_config(&self, host: &mut dyn Host, text: &str, path: PathBuf) {
        let result = self.state().load(text, path);
        let message = match result {
            Ok(()) => None,
            Err(e) => {
                log::warn!("Players Info: config not read, settings kept: {e}");
                let first = e.lines().next().unwrap_or_default().to_owned();
                Some(format!(
                    "Players Info: config error, settings kept ({first})"
                ))
            }
        };
        if let Some(message) = message {
            host.client_action_bar(Text::literal(message), false);
        }
    }
}

/// The player to show this frame, if one qualifies and is in plain view.
fn pick(
    s: &mut State,
    host: &dyn Host,
    camera: &NativeCamera,
    entities: &[NativeClientEntity],
    partial: f64,
) -> Option<u32> {
    let own_uuid = s
        .own_entity
        .and_then(|own| entities.iter().find(|e| e.id == own))
        .and_then(|e| e.player_uuid);
    let spectating = own_uuid.is_some_and(|u| s.sync.listing(u).game_mode == Some(SPECTATOR));
    let candidates: Vec<target::Candidate> = entities
        .iter()
        .filter(|e| e.alive && !e.invisible && Some(e.id) != s.own_entity)
        .filter(|e| {
            e.player_uuid.is_some_and(|uuid| {
                // Spectators are only seen by spectators.
                spectating || s.sync.listing(uuid).game_mode != Some(SPECTATOR)
            })
        })
        .map(|e| target::Candidate {
            id: e.id,
            position: lerp(e.previous_position, e.position, partial),
            radius: ((e.bounds[1][0] - e.bounds[0][0]) * 0.5).clamp(0.1, 2.0),
            height: f64::from(e.height).max(0.2),
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let look = target::forward(camera.yaw, camera.pitch);
    let ranked = target::ranked(
        camera.position,
        look,
        &candidates,
        f64::from(s.config.card.range),
        s.config.card.target,
        camera.fov_degrees,
    );
    ranked.into_iter().take(4).find(|id| {
        candidates.iter().find(|c| c.id == *id).is_some_and(|c| {
            c.sight_points()
                .iter()
                .any(|p| host.client_visual_clear(camera.position, *p))
        })
    })
}

/// Moves the card toward the picked player: it stays a moment after they leave the crosshair,
/// fades its content to change player, and slides out when there's no one.
fn follow(
    s: &mut State,
    entities: &[NativeClientEntity],
    picked: Option<u32>,
    now: f64,
    dt: f32,
    on: bool,
) {
    let linger = f64::from(s.config.card.linger);
    let c = &mut s.card;
    match picked {
        Some(id) => {
            c.wanted = Some(id);
            c.last_seen = now;
        }
        None => {
            let still_here =
                |id: u32| entities.iter().any(|e| e.id == id) && now - c.last_seen <= linger;
            if !on || c.wanted.is_some_and(|id| !still_here(id)) {
                c.wanted = None;
            }
        }
    }
    if c.wanted != c.shown && c.wanted.is_some() {
        if c.shown.is_none() || c.anim.presence < 0.05 {
            c.shown = c.wanted;
            c.anim.restart();
            c.main_hand = None;
        } else {
            c.anim.content_toward(0.0, dt);
            if c.anim.content < 0.05 {
                c.shown = c.wanted;
                c.anim.restart();
                c.main_hand = None;
            }
        }
    } else if c.wanted.is_some() {
        c.anim.content_toward(1.0, dt);
    }
    c.anim.show(c.wanted.is_some(), dt);
    if c.wanted.is_none() && c.anim.presence < 0.005 {
        c.shown = None;
        c.view = None;
        c.main_hand = None;
        c.anchors = None;
    }
}

/// Where the player is on screen: their box, the heights of what their armor covers, and
/// their name tag (a line higher when a score shows under names nearby).
fn anchors(
    s: &State,
    host: &dyn Host,
    camera: &NativeCamera,
    e: &NativeClientEntity,
    name: &str,
    partial: f64,
    screen: [f32; 2],
) -> Option<Anchors> {
    let head = host.client_heads(&[e.id], partial).into_iter().next()?;
    let feet = lerp(e.previous_position, e.position, partial);
    let shift: [f64; 3] = std::array::from_fn(|i| feet[i] - e.position[i]);
    let [lo, hi] = e.bounds;
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for corner in 0..8 {
        let pick = |axis: usize| {
            if corner >> axis & 1 == 0 {
                lo[axis]
            } else {
                hi[axis]
            }
        };
        let at = project(camera, [0, 1, 2].map(|i| pick(i) + shift[i]), screen)?;
        for i in 0..2 {
            min[i] = min[i].min(at[i]);
            max[i] = max[i].max(at[i]);
        }
    }
    let height = f64::from(e.height);
    let part = |share: f64| project(camera, [feet[0], feet[1] + height * share, feet[2]], screen);
    let parts = [
        part(0.87)?[1],
        part(0.62)?[1],
        part(0.35)?[1],
        part(0.07)?[1],
    ];

    let anchor = head.name_anchor;
    let score = s.below_name && distance(camera.position, anchor) < BELOW_NAME_DISTANCE;
    let lift = if score { TAG_LINE } else { 0.0 };
    let up = |by: f64| [anchor[0], anchor[1] + lift + by, anchor[2]];
    let center = up(4.5 * FONT_PIXEL);
    // The game's own width for the tag; the name's when it draws none.
    let half = if head.name_half_width > 0.0 {
        f64::from(head.name_half_width)
    } else {
        f64::from(host.text_width(name)) * FONT_PIXEL * 0.5
    } + FONT_PIXEL;
    let right = right_vector(camera.yaw, camera.pitch);
    let tag = project(camera, center, screen)?;
    let tag_top = project(camera, up(9.5 * FONT_PIXEL), screen)?;
    let tag_end = project(
        camera,
        [
            center[0] + right[0] * half,
            center[1],
            center[2] + right[2] * half,
        ],
        screen,
    )?;
    Some(Anchors {
        tag,
        tag_half_width: (tag_end[0] - tag[0]).abs(),
        tag_height: 2.0 * (tag[1] - tag_top[1]).abs(),
        body: [min[0], min[1], max[0], max[1]],
        parts,
    })
}

/// A world position on screen, in GUI pixels, as the camera draws it this frame; None behind
/// the camera. The view matrix is camera-relative and both are column-major.
fn project(camera: &NativeCamera, at: [f64; 3], screen: [f32; 2]) -> Option<[f32; 2]> {
    let rel = [
        (at[0] - camera.position[0]) as f32,
        (at[1] - camera.position[1]) as f32,
        (at[2] - camera.position[2]) as f32,
        1.0,
    ];
    let times = |m: &[f32; 16], v: [f32; 4]| -> [f32; 4] {
        std::array::from_fn(|row| (0..4).map(|col| m[col * 4 + row] * v[col]).sum())
    };
    let clip = times(&camera.projection, times(&camera.view, rel));
    if clip[3] <= 1e-4 || !clip.iter().all(|v| v.is_finite()) {
        return None;
    }
    let (x, y) = (clip[0] / clip[3], clip[1] / clip[3]);
    Some([(x + 1.0) * 0.5 * screen[0], (1.0 - y) * 0.5 * screen[1]])
}

/// Faces come from the tab list's profiles: asked for every few seconds, or sooner when the
/// player shown has none yet.
fn refresh_skins(s: &mut State, host: &mut dyn Host, uuid: Option<u128>, now: f64) {
    let missing = uuid.is_some_and(|u| !s.skins.contains_key(&u));
    let due = s
        .skins_at
        .is_none_or(|t| now - t > if missing { 0.5 } else { 5.0 });
    if !due {
        return;
    }
    s.skins_at = Some(now);
    s.skins = host
        .client_profiles()
        .into_iter()
        .filter(|p| !p.skin.is_empty())
        .map(|p| (p.uuid, p.skin))
        .collect();
}

fn build_view(
    s: &mut State,
    host: &dyn Host,
    camera: &NativeCamera,
    e: &NativeClientEntity,
    partial: f64,
    now: f64,
) -> View {
    let lang = s.lang();
    let cfg = s.config.card.clone();
    let id = e.id;
    let feet = lerp(e.previous_position, e.position, partial);
    let center = [feet[0], feet[1] + f64::from(e.height) * 0.5, feet[2]];
    let listing = e.player_uuid.map(|u| s.sync.listing(u)).unwrap_or_default();
    let entity = host.entity(id);
    let name = e
        .player_name
        .clone()
        .or_else(|| entity.and_then(|r| host.entity_name(r)))
        .unwrap_or_else(|| "?".into());

    let mut view = View {
        id,
        name,
        skin: e.player_uuid.and_then(|u| s.skins.get(&u).cloned()),
        ..View::default()
    };
    if cfg.game_mode
        && let Some(mode) = listing.game_mode
    {
        view.game_mode = lang.game_mode(mode).map(str::to_owned);
    }
    view.distance = distance(camera.position, center);
    if cfg.ping {
        view.ping = listing.latency;
    }
    if cfg.health
        && let Some(h) = s.health.get_or_full(id, PLAYER)
    {
        view.health = Some(card::Health {
            current: h.current,
            max: h.max(PLAYER),
            absorption: h.absorption,
        });
        view.hearts = HeartStyle {
            hardcore: s.hardcore,
            frozen: s.sync.flags(id).fully_frozen(),
        };
    }

    let worn = equipment(host, entity);
    let main = worn
        .iter()
        .find(|(slot, _)| *slot == EquipmentSlot::MainHand);
    notice_switch(s, host, id, main.map(|(_, stack)| stack), now);
    if cfg.equipment {
        view.items = worn
            .iter()
            .filter_map(|(slot, stack)| {
                CARD_SLOTS
                    .iter()
                    .find(|(_, s)| s == slot)
                    .map(|(card_slot, _)| (*card_slot, stack.clone()))
            })
            .collect();
    }
    if cfg.details {
        (view.main_details, view.off_details) = details(&worn);
    }
    if cfg.state {
        view.state = state(lang, s.sync.flags(id), &worn);
    }
    if cfg.mount
        && let Some(vehicle) = s.sync.vehicle(id)
    {
        let kind = s.kind(host, vehicle);
        if !kind.is_empty() {
            view.mount = Some(vec![
                Span::new(lang.pick("Sur : ", "Riding: "), GRAY),
                Span::new(i18n::entity_name(&kind), WHITE),
            ]);
        }
    }
    let step = [
        e.position[0] - e.previous_position[0],
        e.position[1] - e.previous_position[1],
        e.position[2] - e.previous_position[2],
    ];
    view.speed = (distance([0.0; 3], step) * 20.0) as f32;
    if cfg.coordinates {
        view.position = Some(feet);
    }
    if cfg.events {
        view.events = s
            .feeds
            .recent(id, now)
            .into_iter()
            .take(CARD_EVENTS)
            .map(|event| {
                let (text, color) = event.line(lang);
                EventLine {
                    seq: event.seq,
                    text,
                    color,
                    age: now - event.at,
                    shown_for: now - event.since,
                }
            })
            .collect();
    }
    view.swing = s
        .swings
        .get(&id)
        .map_or(0.0, |t| (1.0 - (now - t) / 0.3).clamp(0.0, 1.0) as f32);
    view
}

/// What an entity wears and holds, slot by slot.
fn equipment(host: &dyn Host, entity: Option<EntityRef>) -> Vec<(EquipmentSlot, ItemStack)> {
    let Some(living) = entity.and_then(|e| host.living(e)) else {
        return Vec::new();
    };
    CARD_SLOTS
        .iter()
        .filter_map(|(_, slot)| {
            host.living_equipment(living, *slot)
                .filter(|stack| stack.count > 0 && &*stack.item != "minecraft:air")
                .map(|stack| (*slot, stack))
        })
        .collect()
}

/// A main-hand change on the player shown is a line in their feed.
fn notice_switch(s: &mut State, host: &dyn Host, id: u32, main: Option<&ItemStack>, now: f64) {
    let item = main.map_or_else(String::new, |stack| stack.item.to_string());
    let before = s.card.main_hand.replace((id, item.clone()));
    let Some((before_id, before_item)) = before else {
        return;
    };
    if before_id != id || before_item == item || s.settling(id, now) || !s.config.card.events {
        return;
    }
    if let Some(stack) = main {
        s.event(host, id, EventKind::Switched(i18n::item_name(stack)), now);
    }
}

/// The held items as their tooltips start: the main hand's name, its enchantments in grey,
/// its durability; and the off hand's name.
fn details(worn: &[(EquipmentSlot, ItemStack)]) -> (Vec<Segments>, Vec<Segments>) {
    let mut lines = Vec::new();
    let held = |slot| {
        worn.iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, stack)| stack)
    };
    if let Some(main) = held(EquipmentSlot::MainHand) {
        lines.push(vec![item_name(main)]);
        // Two enchantments a line, where the tooltip has one: the card stays short.
        for pair in main.enchantments().chunks(2) {
            let mut line: Segments = Vec::new();
            for (i, (id, level)) in pair.iter().enumerate() {
                if i > 0 {
                    line.push(Span::new(", ", GRAY));
                }
                let color = if i18n::is_curse(id) { RED } else { GRAY };
                line.push(Span::new(i18n::enchantment(id, *level), color));
            }
            lines.push(line);
        }
        if let Some(max) = gear::max_durability(&main.item) {
            let left = max.saturating_sub(main.damage);
            lines.push(vec![Span::new(i18n::durability(left, max), WHITE)]);
        }
    }
    let off = held(EquipmentSlot::OffHand)
        .map(|off| vec![vec![item_name(off)]])
        .unwrap_or_default();
    (lines, off)
}

/// An item's name as its tooltip shows it: in its rarity's colour, italic when renamed.
fn item_name(stack: &ItemStack) -> Span {
    let span = Span::new(i18n::item_name(stack), gear::rarity_color(stack));
    if stack.custom_name().is_some() {
        span.italic()
    } else {
        span
    }
}

/// Posture, then what they're doing.
fn state(lang: Lang, flags: sync::Flags, worn: &[(EquipmentSlot, ItemStack)]) -> Segments {
    let mut words: Segments = Vec::new();
    let posture = flags.posture();
    let word = match posture {
        Posture::Crouching => Some(lang.pick("Accroupi", "Sneaking")),
        Posture::Swimming => Some(lang.pick("Nage", "Swimming")),
        Posture::Gliding => Some(lang.pick("Vol en élytres", "Gliding")),
        Posture::Riptide => Some(lang.pick("Tourbillon", "Riptide")),
        Posture::Sleeping => Some(lang.pick("Dort", "Sleeping")),
        Posture::Standing | Posture::Dying => None,
    };
    if let Some(text) = word {
        words.push(Span::new(text, GRAY));
    }
    if flags.sprinting() && matches!(posture, Posture::Standing | Posture::Crouching) {
        words.push(Span::new(lang.pick("Sprint", "Sprinting"), GRAY));
    }
    if flags.using_item() {
        let slot = if flags.using_off_hand() {
            EquipmentSlot::OffHand
        } else {
            EquipmentSlot::MainHand
        };
        if let Some((_, stack)) = worn.iter().find(|(s, _)| *s == slot) {
            words.push(Span::new(gear::use_label(lang, stack), YELLOW));
        }
    }
    if flags.on_fire() {
        words.push(Span::new(lang.pick("En feu", "On fire"), GOLD));
    }
    if flags.glowing() {
        words.push(Span::new(lang.pick("Lumineux", "Glowing"), WHITE));
    }
    words
}

/// The camera's right, flat: text faces the camera as a name tag does
/// (`forward × up`, with yaw 0 looking south and 90 west).
fn right_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    let forward = target::forward(yaw, pitch);
    // forward × (0, 1, 0)
    let r = [-forward[2], 0.0, forward[0]];
    let length = (r[0] * r[0] + r[2] * r[2]).sqrt();
    if length < 1e-6 {
        // Looking straight up or down: the yaw alone says where right is.
        let yaw = f64::from(yaw).to_radians();
        return [-yaw.cos(), 0.0, -yaw.sin()];
    }
    [r[0] / length, 0.0, r[2] / length]
}

fn lerp(from: [f64; 3], to: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    #[test]
    fn right_is_west_when_looking_south_and_north_when_looking_west() {
        assert!(close(right_vector(0.0, 0.0), [-1.0, 0.0, 0.0]));
        assert!(close(right_vector(90.0, 0.0), [0.0, 0.0, -1.0]));
        // Straight down: still a direction, from the yaw.
        assert!(close(right_vector(0.0, 90.0), [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn entity_data_feeds_health_flags_and_pose() {
        let mut s = State::new();
        s.entity_data(
            7,
            &[
                (9, 3, Value::Float(12.0)),
                (17, 3, Value::Float(4.0)),
                (0, 0, Value::Byte(0x08 | 0x01)),
                (6, 20, Value::Id(5)),
                (8, 0, Value::Byte(1)),
            ],
        );
        let h = s.health.get(7).unwrap();
        assert_eq!((h.current, h.absorption), (12.0, 4.0));
        let f = s.sync.flags(7);
        assert!(f.sprinting() && f.on_fire() && f.using_item());
        assert_eq!(f.posture(), Posture::Crouching);
        // Other entity data is ignored, whatever its index.
        s.entity_data(7, &[(9, 1, Value::Int(3))]);
        assert_eq!(s.health.get(7).unwrap().current, 12.0);
    }

    #[test]
    fn state_words_read_in_order() {
        let flags = sync::Flags {
            shared: 0x02 | 0x08 | 0x40,
            pose: 5,
            living: 0x01,
            frozen: 0,
        };
        let shield = ItemStack {
            item: "minecraft:shield".into(),
            count: 1,
            damage: 0,
            extra: None,
        };
        let words: Vec<(String, u32)> =
            state(Lang::En, flags, &[(EquipmentSlot::MainHand, shield)])
                .into_iter()
                .map(|s| (s.text, s.color))
                .collect();
        assert_eq!(
            words,
            [
                ("Sneaking".to_owned(), GRAY),
                ("Sprinting".to_owned(), GRAY),
                ("Blocking".to_owned(), YELLOW),
                ("Glowing".to_owned(), WHITE)
            ]
        );
    }

    #[test]
    fn details_read_as_a_tooltip_does() {
        let sword = ItemStack {
            item: "minecraft:diamond_sword".into(),
            count: 1,
            damage: 61,
            extra: Some(Box::new(horizon_client::mod_api::StackData {
                enchantments: vec![
                    ("minecraft:sharpness".into(), 5),
                    ("minecraft:unbreaking".into(), 3),
                    ("minecraft:vanishing_curse".into(), 1),
                ],
                custom_name: Some("Excalibur".into()),
            })),
        };
        let lines: Vec<Vec<(String, u32, bool)>> = details(&[(EquipmentSlot::MainHand, sword)])
            .0
            .into_iter()
            .map(|l| l.into_iter().map(|s| (s.text, s.color, s.italic)).collect())
            .collect();
        assert_eq!(
            lines[0],
            [("Excalibur".to_owned(), card::color::AQUA, true)]
        );
        assert_eq!(lines[1].len(), 3, "two enchantments a line: {:?}", lines[1]);
        assert_eq!(lines[2][0].1, RED, "a curse is red");
        assert_eq!(lines[3][0].0, "Durability: 1500 / 1561");
    }
}
