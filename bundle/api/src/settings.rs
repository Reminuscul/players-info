//! The page under Options > Mods > Players Info: a button per setting, showing its value; a
//! click moves it to the next one. Colours, symbols and offsets stay in the config file.

use crate::config::{Config, Language, Layout, Placement, Targeting};
use crate::i18n::Lang;

const CARD_RANGES: &[u32] = &[16, 32, 48, 64, 96, 128];
const SCALES: &[f32] = &[0.75, 1.0, 1.25, 1.5];
const OPACITIES: &[f32] = &[0.5, 0.75, 0.9, 1.0];

/// What a button shows, and what a click on it does.
type Label = fn(&Config, Lang) -> String;
type Apply = fn(&mut Config);

/// The buttons, in order.
const BUTTONS: &[(Label, Apply)] = &[
    (
        |c, l| format!("Players Info : {}", l.on_off(c.enabled)),
        |c| c.enabled ^= true,
    ),
    (
        |c, l| {
            let name = match c.language {
                Language::Auto => l.pick("Auto (langue du jeu)", "Auto (game's)"),
                Language::Fr => "Français",
                Language::En => "English",
            };
            format!("{} : {name}", l.pick("Langue", "Language"))
        },
        |c| c.language = cycle(&Language::ALL, c.language),
    ),
    // The card.
    (
        |c, l| {
            on_off(
                l,
                l.pick("Panneau de la cible", "Target card"),
                c.card.enabled,
            )
        },
        |c| c.card.enabled ^= true,
    ),
    (
        |c, l| {
            let layout = match c.card.layout {
                Layout::Around => l.pick("autour du joueur", "around the player"),
                Layout::Panel => l.pick("infobulle", "tooltip"),
            };
            format!("{} : {layout}", l.pick("Disposition", "Layout"))
        },
        |c| {
            c.card.layout = match c.card.layout {
                Layout::Around => Layout::Panel,
                Layout::Panel => Layout::Around,
            }
        },
    ),
    (
        |c, l| {
            let place = match c.card.placement {
                Placement::Right => l.pick("à droite", "right"),
                Placement::Left => l.pick("à gauche", "left"),
                Placement::Top => l.pick("en haut", "top"),
                Placement::Crosshair => l.pick("près du viseur", "by the crosshair"),
            };
            format!("{} : {place}", l.pick("Infobulle", "Tooltip"))
        },
        |c| c.card.placement = cycle(&Placement::ALL, c.card.placement),
    ),
    (
        |c, l| {
            let target = match c.card.target {
                Targeting::Crosshair => l.pick("sous le viseur", "under the crosshair"),
                Targeting::Nearest => l.pick("le plus proche", "nearest"),
            };
            format!("{} : {target}", l.pick("Joueur suivi", "Follows"))
        },
        |c| {
            c.card.target = match c.card.target {
                Targeting::Crosshair => Targeting::Nearest,
                Targeting::Nearest => Targeting::Crosshair,
            }
        },
    ),
    (
        |c, l| {
            format!(
                "{} : {} {}",
                l.pick("Portée", "Range"),
                c.card.range,
                l.pick("blocs", "blocks")
            )
        },
        |c| c.card.range = next(CARD_RANGES, c.card.range),
    ),
    (
        |c, l| {
            format!(
                "{} : {}%",
                l.pick("Taille", "Size"),
                (c.card.scale * 100.0).round()
            )
        },
        |c| c.card.scale = next(SCALES, c.card.scale),
    ),
    (
        |c, l| {
            format!(
                "{} : {}%",
                l.pick("Fond", "Background"),
                (c.card.opacity * 100.0).round()
            )
        },
        |c| c.card.opacity = next(OPACITIES, c.card.opacity),
    ),
    (
        |c, l| on_off(l, l.pick("Mode de jeu", "Game mode"), c.card.game_mode),
        |c| c.card.game_mode ^= true,
    ),
    (
        |c, l| on_off(l, "Ping", c.card.ping),
        |c| c.card.ping ^= true,
    ),
    (
        |c, l| on_off(l, l.pick("Santé", "Health"), c.card.health),
        |c| c.card.health ^= true,
    ),
    (
        |c, l| on_off(l, l.pick("Équipement", "Equipment"), c.card.equipment),
        |c| c.card.equipment ^= true,
    ),
    (
        |c, l| {
            on_off(
                l,
                l.pick("Noms et enchantements", "Names and enchantments"),
                c.card.details,
            )
        },
        |c| c.card.details ^= true,
    ),
    (
        |c, l| {
            on_off(
                l,
                l.pick("Posture et actions", "Posture and actions"),
                c.card.state,
            )
        },
        |c| c.card.state ^= true,
    ),
    (
        |c, l| on_off(l, l.pick("Monture", "Mount"), c.card.mount),
        |c| c.card.mount ^= true,
    ),
    (
        |c, l| on_off(l, l.pick("Coordonnées", "Coordinates"), c.card.coordinates),
        |c| c.card.coordinates ^= true,
    ),
    (
        |c, l| on_off(l, l.pick("Événements", "Events"), c.card.events),
        |c| c.card.events ^= true,
    ),
];

pub fn labels(config: &Config) -> Vec<String> {
    let lang = Lang::of(config.language);
    BUTTONS
        .iter()
        .map(|(label, _)| {
            let text = label(config, lang);
            // `Name : value` in French, `Name: value` in English.
            match lang {
                Lang::Fr => text,
                Lang::En => text.replacen(" : ", ": ", 1),
            }
        })
        .collect()
}

/// Applies a click on button `index`; false when there is no such button.
pub fn click(config: &mut Config, index: usize) -> bool {
    match BUTTONS.get(index) {
        Some((_, apply)) => {
            apply(config);
            true
        }
        None => false,
    }
}

fn on_off(lang: Lang, name: &str, on: bool) -> String {
    format!("{name} : {}", lang.on_off(on))
}

fn cycle<T: PartialEq + Copy>(all: &[T], current: T) -> T {
    let i = all.iter().position(|v| *v == current).unwrap_or(0);
    all[(i + 1) % all.len()]
}

/// The step after `current`, wrapping; for a value typed in the file, the next one up.
fn next<T: PartialOrd + Copy>(steps: &[T], current: T) -> T {
    match steps.iter().position(|s| *s == current) {
        Some(i) => steps[(i + 1) % steps.len()],
        None => steps
            .iter()
            .copied()
            .find(|s| *s > current)
            .unwrap_or(steps[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_changes_what_it_shows() {
        let mut config = Config::initial();
        config.language = Language::En;
        for i in 0..labels(&config).len() {
            let before = labels(&config)[i].clone();
            assert!(click(&mut config, i));
            assert_ne!(labels(&config)[i], before, "button {i}");
            // Back to English for the next buttons' labels.
            config.language = Language::En;
        }
        assert!(!click(&mut config, BUTTONS.len()));
    }

    #[test]
    fn labels_speak_the_chosen_language() {
        let mut config = Config::initial();
        config.language = Language::Fr;
        assert!(labels(&config).contains(&"Ping : OUI".to_owned()));
        config.language = Language::En;
        let labels = labels(&config);
        assert!(labels.contains(&"Ping: ON".to_owned()));
        assert!(labels.contains(&"Range: 48 blocks".to_owned()));
        assert!(labels.contains(&"Layout: around the player".to_owned()));
    }

    #[test]
    fn steps_wrap_and_odd_values_move_up() {
        assert_eq!(next(CARD_RANGES, 128), 16);
        assert_eq!(next(CARD_RANGES, 20), 32);
        assert_eq!(next(SCALES, 1.1), 1.25);
        assert_eq!(next(OPACITIES, 1.0), 0.5);
        assert_eq!(next(OPACITIES, 0.72), 0.75);
    }
}
