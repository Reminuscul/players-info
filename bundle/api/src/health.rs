//! What the client knows of each entity's health, from the server's entity data packets.
//!
//! The server sends a living entity's health (`LivingEntity.DATA_HEALTH_ID`) and a player's
//! absorption hearts (`Player.DATA_PLAYER_ABSORPTION_ID`) when they change; an entity whose
//! health never came is at its maximum, as the game holds it (`Tracker::get_or_full`). Its
//! maximum health is an attribute the client can't name without the server's registry, so the
//! maximum is the larger of the type's vanilla default and the most health seen: right for
//! every vanilla mob at full health, and right for a boosted one as soon as it has been seen at
//! full health.

use std::collections::HashMap;

/// Entity data indexes, as at Minecraft 26.3 (the same as Horizon's `remote_mob_data.rs`).
pub const DATA_HEALTH: u8 = 9;
pub const DATA_PLAYER_ABSORPTION: u8 = 17;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Health {
    pub current: f32,
    /// The highest health seen for this entity.
    pub seen_max: f32,
    /// Golden hearts, for players (0 for anything else).
    pub absorption: f32,
    /// The server sent the health; until then `current` means nothing.
    pub known: bool,
}

#[derive(Debug, Default)]
pub struct Tracker {
    entities: HashMap<u32, Health>,
}

impl Tracker {
    pub fn set_health(&mut self, id: u32, health: f32) {
        if !health.is_finite() {
            return;
        }
        let h = self.entities.entry(id).or_default();
        h.current = health.max(0.0);
        h.seen_max = h.seen_max.max(h.current);
        h.known = true;
    }

    /// Only meaningful for players: other entities use the same index for something else, so
    /// the caller decides whether to show it.
    pub fn set_absorption(&mut self, id: u32, amount: f32) {
        if amount.is_finite() {
            self.entities.entry(id).or_default().absorption = amount.max(0.0);
        }
    }

    pub fn remove(&mut self, id: u32) {
        self.entities.remove(&id);
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }

    pub fn get(&self, id: u32) -> Option<Health> {
        self.entities.get(&id).copied()
    }

    /// Entity `id`'s health as the game itself holds it. Data the server leaves out is the
    /// entity's own default, and a new living entity starts at its maximum health
    /// (`LivingEntity`'s constructor; Horizon's mobs start at their type's `max_health`): a
    /// player whose health hasn't come is at full health, not unknown. That is the common case
    /// for a player at full health, whose hearts would otherwise wait for their first change (a
    /// hit). None only for a type whose maximum isn't known.
    pub fn get_or_full(&self, id: u32, kind: &str) -> Option<Health> {
        let mut h = self.get(id).unwrap_or_default();
        if !h.known {
            let full = default_max_health(kind)?;
            h.current = full;
            h.seen_max = h.seen_max.max(full);
        }
        Some(h)
    }
}

impl Health {
    /// The maximum to show against, for an entity of type `kind`; None when unknown (a type
    /// not in the table, never seen above its current health).
    pub fn max(&self, kind: &str) -> Option<f32> {
        let default = default_max_health(kind);
        let seen = (self.seen_max > self.current).then_some(self.seen_max);
        match (default, seen) {
            (Some(d), Some(s)) => Some(d.max(s)),
            (Some(d), None) => Some(d.max(self.current)),
            (None, s) => s,
        }
    }
}

/// `Attributes.MAX_HEALTH` of vanilla's entity types, by their defaults
/// (`DefaultAttributes`). Types whose maximum varies (horses, slimes, llamas) are left out.
pub fn default_max_health(kind: &str) -> Option<f32> {
    let name = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let max = match name {
        "player" | "zombie" | "husk" | "drowned" | "zombie_villager" | "zombified_piglin"
        | "skeleton" | "stray" | "creeper" | "blaze" | "wither_skeleton" | "phantom"
        | "villager" | "wandering_trader" | "panda" | "strider" | "allay" | "happy_ghast" => 20.0,
        "spider" | "piglin" | "bogged" => 16.0,
        "cave_spider" | "armadillo" | "copper_golem" => 12.0,
        "enderman" | "hoglin" | "zoglin" => 40.0,
        "witch" => 26.0,
        "ghast" | "cow" | "mooshroom" | "pig" | "cat" | "ocelot" | "fox" | "bee" | "goat"
        | "frog" | "dolphin" | "squid" | "glow_squid" | "pufferfish" => 10.0,
        "guardian" | "shulker" | "breeze" | "turtle" | "polar_bear" => 30.0,
        "elder_guardian" => 80.0,
        "vindicator" | "evoker" | "pillager" => 24.0,
        "illusioner" => 32.0,
        "piglin_brute" => 50.0,
        "ravager" | "iron_golem" => 100.0,
        "vex" | "axolotl" | "sniffer" => 14.0,
        "silverfish" | "endermite" | "sheep" | "wolf" => 8.0,
        "warden" => 500.0,
        "wither" => 300.0,
        "ender_dragon" => 200.0,
        "snow_golem" | "chicken" => 4.0,
        "bat" | "parrot" => 6.0,
        "rabbit" | "cod" | "salmon" | "tropical_fish" | "tadpole" => 3.0,
        "camel" => 32.0,
        _ => return None,
    };
    Some(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_follows_the_packets_and_remembers_its_peak() {
        let mut t = Tracker::default();
        t.set_health(1, 20.0);
        t.set_health(1, 13.5);
        let h = t.get(1).unwrap();
        assert_eq!((h.current, h.seen_max), (13.5, 20.0));
        t.set_health(1, f32::NAN);
        assert_eq!(t.get(1).unwrap().current, 13.5);
        t.remove(1);
        assert_eq!(t.get(1), None);
    }

    #[test]
    fn a_player_whose_health_never_came_is_at_full_health() {
        let mut t = Tracker::default();
        let full = t.get_or_full(1, "minecraft:player").unwrap();
        assert_eq!(
            (full.current, full.max("minecraft:player")),
            (20.0, Some(20.0))
        );
        // The absorption came without the health: still at full health, with its golden hearts.
        t.set_absorption(2, 4.0);
        let golden = t.get_or_full(2, "minecraft:player").unwrap();
        assert_eq!((golden.current, golden.absorption), (20.0, 4.0));
        t.set_health(2, 7.0);
        assert_eq!(t.get_or_full(2, "minecraft:player").unwrap().current, 7.0);
        assert_eq!(t.get_or_full(3, "minecraft:breeze_wind_charge"), None);
    }

    #[test]
    fn the_maximum_is_the_types_unless_more_was_seen() {
        let damaged_zombie = Health {
            current: 7.0,
            seen_max: 7.0,
            absorption: 0.0,
            known: true,
        };
        assert_eq!(damaged_zombie.max("minecraft:zombie"), Some(20.0));
        // A player with Health Boost seen at 28.
        let boosted = Health {
            current: 24.0,
            seen_max: 28.0,
            absorption: 0.0,
            known: true,
        };
        assert_eq!(boosted.max("minecraft:player"), Some(28.0));
        // A horse's maximum varies: known only once seen above its health.
        let horse = Health {
            current: 21.0,
            seen_max: 21.0,
            absorption: 0.0,
            known: true,
        };
        assert_eq!(horse.max("minecraft:horse"), None);
        let hurt_horse = Health {
            current: 10.0,
            seen_max: 21.0,
            absorption: 0.0,
            known: true,
        };
        assert_eq!(hurt_horse.max("minecraft:horse"), Some(21.0));
    }
}
