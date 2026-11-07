//! What just happened to a player, from the packets the server sends about them: hits taken,
//! critical hits, totems, shields, deaths, mounts, game mode and weapon changes. Each player
//! keeps a short feed; repeats close together fold into one line (`Hit by Alex ×3`).

use crate::card::color::{AQUA, GOLD, GRAY, GREEN, LIGHT_PURPLE, RED, WHITE, YELLOW};
use crate::i18n::Lang;
use std::collections::{HashMap, VecDeque};

/// Seconds a line stays in the feed.
pub const LIFETIME: f64 = 6.0;
/// Lines kept per player.
const KEPT: usize = 4;
/// Two hits closer than this are one line.
const FOLD: f64 = 1.5;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Hurt { by: Option<String> },
    Critical,
    MagicCritical,
    Totem,
    Blocked,
    ShieldDisabled,
    Died,
    Mounted(String),
    Dismounted(String),
    GameMode(i32),
    Switched(String),
    Teleported,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub kind: Kind,
    /// When it last happened (seconds on the mod's clock).
    pub at: f64,
    /// When the line appeared; it slides in from there.
    pub since: f64,
    pub count: u32,
    /// Unique and increasing: what the card animates a line by.
    pub seq: u64,
}

#[derive(Debug, Default)]
pub struct Feeds {
    feeds: HashMap<u32, VecDeque<Event>>,
    next_seq: u64,
}

impl Feeds {
    pub fn push(&mut self, id: u32, kind: Kind, now: f64) {
        let feed = self.feeds.entry(id).or_default();
        if let Some(last) = feed.back_mut()
            && now - last.at < FOLD
            && folds(&last.kind, &kind)
        {
            // The damage packet and the hurt animation of one hit: still one hit.
            if !(matches!(kind, Kind::Hurt { .. }) && now - last.at < 0.15) {
                last.count += 1;
            }
            last.at = now;
            if let (Kind::Hurt { by: None }, Kind::Hurt { by: Some(_) }) = (&last.kind, &kind) {
                last.kind = kind;
            } else if matches!(kind, Kind::Switched(_)) {
                last.kind = kind;
                last.count = 1;
            }
            return;
        }
        self.next_seq += 1;
        feed.push_back(Event {
            kind,
            at: now,
            since: now,
            count: 1,
            seq: self.next_seq,
        });
        while feed.len() > KEPT {
            feed.pop_front();
        }
    }

    /// The player's lines still in their lifetime, newest first.
    pub fn recent(&self, id: u32, now: f64) -> Vec<&Event> {
        self.feeds.get(&id).map_or_else(Vec::new, |feed| {
            feed.iter()
                .rev()
                .filter(|e| now - e.at < LIFETIME)
                .collect()
        })
    }

    /// Drops lines past their lifetime, and players with none left.
    pub fn prune(&mut self, now: f64) {
        self.feeds.retain(|_, feed| {
            feed.retain(|e| now - e.at < LIFETIME);
            !feed.is_empty()
        });
    }

    pub fn remove(&mut self, id: u32) {
        self.feeds.remove(&id);
    }

    pub fn clear(&mut self) {
        self.feeds.clear();
    }
}

/// Whether `next` repeats `last` and joins its line.
fn folds(last: &Kind, next: &Kind) -> bool {
    match (last, next) {
        (Kind::Hurt { by: a }, Kind::Hurt { by: b }) => a == b || a.is_none() || b.is_none(),
        (Kind::Switched(_), Kind::Switched(_)) => true,
        (a, b) => a == b,
    }
}

impl Event {
    /// The line's text and colour (`0xRRGGBB`).
    pub fn line(&self, lang: Lang) -> (String, u32) {
        let (text, color) = match &self.kind {
            Kind::Hurt { by: Some(name) } => {
                (format!("{} {name}", lang.pick("Touché par", "Hit by")), RED)
            }
            Kind::Hurt { by: None } => (lang.pick("Touché", "Hit").to_owned(), RED),
            Kind::Critical => (
                lang.pick("Coup critique subi", "Took a critical hit")
                    .to_owned(),
                GOLD,
            ),
            Kind::MagicCritical => (
                lang.pick("Coup enchanté subi", "Took an enchanted hit")
                    .to_owned(),
                LIGHT_PURPLE,
            ),
            Kind::Totem => (
                lang.pick("Totem d'immortalité utilisé", "Totem of Undying popped")
                    .to_owned(),
                YELLOW,
            ),
            Kind::Blocked => (
                lang.pick("Coup bloqué au bouclier", "Blocked a hit")
                    .to_owned(),
                AQUA,
            ),
            Kind::ShieldDisabled => (
                lang.pick("Bouclier désactivé", "Shield disabled")
                    .to_owned(),
                GOLD,
            ),
            Kind::Died => (lang.pick("Mort", "Died").to_owned(), RED),
            Kind::Mounted(what) => (
                format!("{} {what}", lang.pick("Monte sur", "Got on")),
                GREEN,
            ),
            Kind::Dismounted(what) => (
                format!("{} {what}", lang.pick("Descend de", "Got off")),
                GRAY,
            ),
            Kind::GameMode(mode) => (
                format!(
                    "{} {}",
                    lang.pick("Passe en", "Now in"),
                    lang.game_mode(*mode).unwrap_or("?")
                ),
                YELLOW,
            ),
            Kind::Switched(item) => (
                format!("{} {item}", lang.pick("Prend", "Switched to")),
                WHITE,
            ),
            Kind::Teleported => (
                lang.pick("Se téléporte", "Teleported").to_owned(),
                LIGHT_PURPLE,
            ),
        };
        if self.count > 1 {
            (format!("{text} ×{}", self.count), color)
        } else {
            (text, color)
        }
    }
}

/// `EntityEvent` ids worth a line (`EntityEvent.java`).
pub fn from_entity_event(event: i8) -> Option<Kind> {
    match event {
        3 => Some(Kind::Died),
        29 => Some(Kind::Blocked),
        30 => Some(Kind::ShieldDisabled),
        35 => Some(Kind::Totem),
        46 => Some(Kind::Teleported),
        _ => None,
    }
}

/// `ClientboundAnimatePacket` actions: 0 and 3 swing an arm, 4 and 5 are the crit particles
/// on the entity hit.
pub fn from_animation(action: u8) -> Option<Kind> {
    match action {
        4 => Some(Kind::Critical),
        5 => Some(Kind::MagicCritical),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_close_together_fold_and_take_the_attackers_name() {
        let mut f = Feeds::default();
        f.push(1, Kind::Hurt { by: None }, 10.0);
        // The same hit's damage packet, naming who.
        f.push(
            1,
            Kind::Hurt {
                by: Some("Alex".into()),
            },
            10.05,
        );
        f.push(
            1,
            Kind::Hurt {
                by: Some("Alex".into()),
            },
            10.8,
        );
        let recent = f.recent(1, 11.0);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].line(Lang::En).0, "Hit by Alex ×2");
        // Later, a new line.
        f.push(
            1,
            Kind::Hurt {
                by: Some("Alex".into()),
            },
            13.0,
        );
        assert_eq!(f.recent(1, 13.0).len(), 2);
    }

    #[test]
    fn lines_expire_and_the_feed_stays_short() {
        let mut f = Feeds::default();
        for (i, kind) in [
            Kind::Totem,
            Kind::Blocked,
            Kind::Died,
            Kind::Critical,
            Kind::Teleported,
        ]
        .into_iter()
        .enumerate()
        {
            f.push(1, kind, i as f64 * 2.0);
        }
        assert_eq!(f.recent(1, 8.0).len(), 3);
        assert_eq!(f.recent(1, 8.0)[0].kind, Kind::Teleported);
        f.prune(100.0);
        assert!(f.recent(1, 100.0).is_empty());
        assert!(f.feeds.is_empty());
    }

    #[test]
    fn a_weapon_change_replaces_the_one_before() {
        let mut f = Feeds::default();
        f.push(2, Kind::Switched("Bow".into()), 1.0);
        f.push(2, Kind::Switched("Sword".into()), 1.4);
        let recent = f.recent(2, 1.5);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].line(Lang::Fr).0, "Prend Sword");
    }

    #[test]
    fn packets_map_to_lines() {
        assert_eq!(from_entity_event(35), Some(Kind::Totem));
        assert_eq!(from_entity_event(2), None);
        assert_eq!(from_animation(4), Some(Kind::Critical));
        assert_eq!(from_animation(0), None);
        assert_eq!(
            Event {
                kind: Kind::GameMode(1),
                at: 0.0,
                since: 0.0,
                count: 1,
                seq: 1
            }
            .line(Lang::Fr)
            .0,
            "Passe en Créatif"
        );
    }
}
