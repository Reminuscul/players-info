//! What the server syncs about each entity beyond its health: its shared flags, pose and
//! item use (entity data), the vehicle it rides (passenger packets), and, for players, the
//! tab list's game mode and latency.

use std::collections::HashMap;

/// Entity data indexes, as at Minecraft 26.3 (Horizon's `remote_mob_data.rs`).
/// `Entity.DATA_SHARED_FLAGS_ID`.
pub const DATA_FLAGS: u8 = 0;
/// `Entity.DATA_POSE`.
pub const DATA_POSE: u8 = 6;
/// `Entity.DATA_TICKS_FROZEN`.
pub const DATA_TICKS_FROZEN: u8 = 7;
/// `LivingEntity.DATA_LIVING_ENTITY_FLAGS`.
pub const DATA_LIVING_FLAGS: u8 = 8;
/// `Entity.getTicksRequiredToFreeze`: frozen solid from here (the frozen hearts).
const FROZEN_SOLID: i32 = 140;

/// `Pose` network ids.
const POSE_FALL_FLYING: i32 = 1;
const POSE_SLEEPING: i32 = 2;
const POSE_SWIMMING: i32 = 3;
const POSE_SPIN_ATTACK: i32 = 4;
const POSE_CROUCHING: i32 = 5;
const POSE_DYING: i32 = 7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// `DATA_SHARED_FLAGS_ID`: on fire 1, crouching 2, sprinting 8, swimming 16, invisible 32,
    /// glowing 64, fall flying 128.
    pub shared: u8,
    pub pose: i32,
    /// Using an item 1, with the off hand 2, riptide 4.
    pub living: u8,
    /// Ticks spent in powder snow.
    pub frozen: i32,
}

/// The body's posture, one at a time, as the model shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posture {
    Standing,
    Crouching,
    Swimming,
    Gliding,
    Riptide,
    Sleeping,
    Dying,
}

impl Flags {
    pub fn on_fire(&self) -> bool {
        self.shared & 0x01 != 0
    }
    pub fn sprinting(&self) -> bool {
        self.shared & 0x08 != 0
    }
    pub fn glowing(&self) -> bool {
        self.shared & 0x40 != 0
    }
    pub fn using_item(&self) -> bool {
        self.living & 0x01 != 0
    }
    pub fn using_off_hand(&self) -> bool {
        self.living & 0x02 != 0
    }
    /// `Entity.isFullyFrozen`.
    pub fn fully_frozen(&self) -> bool {
        self.frozen >= FROZEN_SOLID
    }

    pub fn posture(&self) -> Posture {
        match self.pose {
            POSE_SLEEPING => Posture::Sleeping,
            POSE_DYING => Posture::Dying,
            POSE_FALL_FLYING => Posture::Gliding,
            POSE_SPIN_ATTACK => Posture::Riptide,
            POSE_SWIMMING => Posture::Swimming,
            POSE_CROUCHING => Posture::Crouching,
            _ if self.shared & 0x80 != 0 => Posture::Gliding,
            _ if self.living & 0x04 != 0 => Posture::Riptide,
            _ if self.shared & 0x02 != 0 => Posture::Crouching,
            _ => Posture::Standing,
        }
    }
}

/// A player's line in the tab list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub game_mode: Option<i32>,
    /// Milliseconds, as the server measures them (0 before its first keep-alive).
    pub latency: Option<i32>,
}

#[derive(Debug, Default)]
pub struct Sync {
    flags: HashMap<u32, Flags>,
    /// Passenger to the vehicle it rides.
    vehicle_of: HashMap<u32, u32>,
    listings: HashMap<u128, Listing>,
}

/// Who got on or off what, from one passenger packet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Boarding {
    pub on: Vec<u32>,
    pub off: Vec<u32>,
}

impl Sync {
    pub fn flags(&self, id: u32) -> Flags {
        self.flags.get(&id).copied().unwrap_or_default()
    }

    pub fn set_shared(&mut self, id: u32, flags: i8) {
        self.flags.entry(id).or_default().shared = flags as u8;
    }
    pub fn set_pose(&mut self, id: u32, pose: i32) {
        self.flags.entry(id).or_default().pose = pose;
    }
    pub fn set_living(&mut self, id: u32, flags: i8) {
        self.flags.entry(id).or_default().living = flags as u8;
    }
    pub fn set_frozen(&mut self, id: u32, ticks: i32) {
        self.flags.entry(id).or_default().frozen = ticks;
    }

    pub fn vehicle(&self, passenger: u32) -> Option<u32> {
        self.vehicle_of.get(&passenger).copied()
    }

    /// `vehicle` now carries exactly `passengers`.
    pub fn set_passengers(&mut self, vehicle: u32, passengers: &[u32]) -> Boarding {
        let mut boarding = Boarding::default();
        let before: Vec<u32> = self
            .vehicle_of
            .iter()
            .filter(|(_, v)| **v == vehicle)
            .map(|(p, _)| *p)
            .collect();
        for p in before {
            if !passengers.contains(&p) {
                self.vehicle_of.remove(&p);
                boarding.off.push(p);
            }
        }
        for &p in passengers {
            if self.vehicle_of.insert(p, vehicle) != Some(vehicle) {
                boarding.on.push(p);
            }
        }
        boarding.off.sort_unstable();
        boarding
    }

    pub fn listing(&self, uuid: u128) -> Listing {
        self.listings.get(&uuid).copied().unwrap_or_default()
    }

    /// Applies a tab list entry's fields; returns the new game mode when it changed from a
    /// known one.
    pub fn update_listing(
        &mut self,
        uuid: u128,
        game_mode: Option<i32>,
        latency: Option<i32>,
    ) -> Option<i32> {
        let listing = self.listings.entry(uuid).or_default();
        let mut changed = None;
        if let Some(mode) = game_mode {
            if listing.game_mode.is_some_and(|old| old != mode) {
                changed = Some(mode);
            }
            listing.game_mode = Some(mode);
        }
        if let Some(latency) = latency {
            listing.latency = Some(latency);
        }
        changed
    }

    pub fn remove_listing(&mut self, uuid: u128) {
        self.listings.remove(&uuid);
    }

    /// The entity left the client's world: its flags, its seat and its passengers' seats go.
    pub fn remove_entity(&mut self, id: u32) {
        self.flags.remove(&id);
        self.vehicle_of.remove(&id);
        self.vehicle_of.retain(|_, v| *v != id);
    }

    /// A new world (login, respawn, level change): entities go, the tab list stays.
    pub fn clear_world(&mut self) {
        self.flags.clear();
        self.vehicle_of.clear();
    }

    pub fn clear(&mut self) {
        self.clear_world();
        self.listings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pose_wins_over_the_flags_and_flags_fill_in() {
        let mut f = Flags::default();
        assert_eq!(f.posture(), Posture::Standing);
        f.shared = 0x02;
        assert_eq!(f.posture(), Posture::Crouching);
        f.pose = POSE_SWIMMING;
        assert_eq!(f.posture(), Posture::Swimming);
        f.pose = 0;
        f.shared = 0x80u8 | 0x08;
        assert_eq!(f.posture(), Posture::Gliding);
        assert!(f.sprinting());
        f = Flags {
            living: 0x03,
            frozen: 140,
            ..Flags::default()
        };
        assert!(f.using_item() && f.using_off_hand() && f.fully_frozen());
    }

    #[test]
    fn passengers_get_on_and_off() {
        let mut s = Sync::default();
        let b = s.set_passengers(10, &[1, 2]);
        assert_eq!(b.on, [1, 2]);
        assert_eq!(s.vehicle(2), Some(10));
        let b = s.set_passengers(10, &[2]);
        assert_eq!((b.on, b.off), (vec![], vec![1]));
        assert_eq!(s.vehicle(1), None);
        // The vehicle goes: its passenger is on foot.
        s.remove_entity(10);
        assert_eq!(s.vehicle(2), None);
    }

    #[test]
    fn a_game_mode_change_is_noticed_only_from_a_known_one() {
        let mut s = Sync::default();
        assert_eq!(s.update_listing(7, Some(0), Some(30)), None);
        assert_eq!(s.update_listing(7, None, Some(45)), None);
        assert_eq!(s.listing(7).latency, Some(45));
        assert_eq!(s.update_listing(7, Some(1), None), Some(1));
        assert_eq!(s.listing(7).game_mode, Some(1));
    }
}
