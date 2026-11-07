//! `config/players_info-client.toml`. Every key is optional; values outside their range are
//! brought back into it. Written back, comments included, when a setting changes in game.

use serde::Deserialize;
use std::fmt::Write as _;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub language: Language,
    pub card: Card,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// The game's: French in French, English otherwise.
    #[default]
    Auto,
    Fr,
    En,
}

/// How the card is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// Around the player, following them on screen.
    #[default]
    Around,
    /// In a tooltip box at `placement`.
    Panel,
}

impl Layout {
    pub fn name(self) -> &'static str {
        match self {
            Layout::Around => "around",
            Layout::Panel => "panel",
        }
    }
}

/// Where the panel sits on screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// Middle of the right edge.
    #[default]
    Right,
    Left,
    /// Top centre, under the boss bars.
    Top,
    /// Beside the crosshair.
    Crosshair,
}

/// Which player the card is about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Targeting {
    /// The one under the crosshair.
    #[default]
    Crosshair,
    /// The nearest one in view.
    Nearest,
}

/// The card: everything about the player looked at, on the HUD.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Card {
    pub enabled: bool,
    pub layout: Layout,
    pub placement: Placement,
    pub offset_x: i32,
    pub offset_y: i32,
    pub scale: f32,
    /// The background's opacity, 0 to 1.
    pub opacity: f32,
    pub target: Targeting,
    /// Blocks.
    pub range: u32,
    /// Seconds the card stays after the player leaves the crosshair.
    pub linger: f32,
    pub game_mode: bool,
    pub ping: bool,
    pub health: bool,
    pub equipment: bool,
    /// Item names, enchantments and durability.
    pub details: bool,
    /// Posture and what they are doing (sprinting, eating, blocking...).
    pub state: bool,
    pub mount: bool,
    pub coordinates: bool,
    pub events: bool,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: Layout::Around,
            placement: Placement::Right,
            offset_x: 0,
            offset_y: 0,
            scale: 1.0,
            opacity: 1.0,
            target: Targeting::Crosshair,
            range: 48,
            linger: 1.5,
            game_mode: true,
            ping: true,
            health: true,
            equipment: true,
            details: true,
            state: true,
            mount: true,
            coordinates: true,
            events: true,
        }
    }
}

impl Placement {
    pub const ALL: [Placement; 4] = [
        Placement::Right,
        Placement::Left,
        Placement::Top,
        Placement::Crosshair,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Placement::Right => "right",
            Placement::Left => "left",
            Placement::Top => "top",
            Placement::Crosshair => "crosshair",
        }
    }
}

impl Targeting {
    pub fn name(self) -> &'static str {
        match self {
            Targeting::Crosshair => "crosshair",
            Targeting::Nearest => "nearest",
        }
    }
}

impl Language {
    pub const ALL: [Language; 3] = [Language::Auto, Language::Fr, Language::En];

    pub fn name(self) -> &'static str {
        match self {
            Language::Auto => "auto",
            Language::Fr => "fr",
            Language::En => "en",
        }
    }
}

impl Config {
    /// The config a fresh install starts with: on.
    pub fn initial() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Parses a config file.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut config: Config = if text.trim().is_empty() {
            Config::initial()
        } else {
            let mut table: toml::Table =
                text.parse().map_err(|e: toml::de::Error| e.to_string())?;
            // `enabled` defaults to true too, when the rest of the file is there.
            table.entry("enabled").or_insert(true.into());
            table
                .try_into()
                .map_err(|e: toml::de::Error| e.to_string())?
        };
        let card = &mut config.card;
        card.range = card.range.clamp(4, 128);
        card.scale = finite_or(card.scale, 1.0).clamp(0.5, 2.0);
        card.opacity = finite_or(card.opacity, 1.0).clamp(0.0, 1.0);
        card.linger = finite_or(card.linger, 1.5).clamp(0.0, 10.0);
        card.offset_x = card.offset_x.clamp(-2000, 2000);
        card.offset_y = card.offset_y.clamp(-2000, 2000);
        Ok(config)
    }

    pub fn to_toml(&self) -> String {
        let c = &self.card;
        let mut out = String::new();
        let mut line = |text: String| {
            out.push_str(&text);
            out.push('\n');
        };
        line("# Players Info 1.0 : les infos du joueur que tu regardes, autour de lui.".into());
        line("# Enregistré quand un réglage change en jeu (Options > Mods >".into());
        line(
            "# Players Info) ; une modification ici est prise en compte en moins d'une seconde."
                .into(),
        );
        line(String::new());
        line(format!("enabled = {}", self.enabled));
        line("# auto (la langue du jeu), fr ou en.".into());
        line(format!("language = {}\n", quote(self.language.name())));

        line("# Les infos sur le joueur visé, s'il est en vue (pas à travers les murs).".into());
        line("[card]".into());
        line(format!("enabled = {}", c.enabled));
        line("# around : autour du joueur, qui suivent ses mouvements à l'écran ;".into());
        line("# panel : dans une infobulle, à la place de `placement`.".into());
        line(format!("layout = {}", quote(c.layout.name())));
        line(
            "# Infobulle : right, left, top (sous les barres de boss) ou crosshair (près du viseur)."
                .into(),
        );
        line(format!("placement = {}", quote(c.placement.name())));
        line("# Infobulle : décalage en pixels d'interface.".into());
        line(format!("offset_x = {}", c.offset_x));
        line(format!("offset_y = {}", c.offset_y));
        line("# Taille, 0.5 à 2.".into());
        line(format!("scale = {}", float(c.scale)));
        line("# Opacité du fond de l'infobulle, 0 à 1 (1 : celle du jeu).".into());
        line(format!("opacity = {}", float(c.opacity)));
        line("# crosshair (le joueur sous le viseur) ou nearest (le plus proche en vue).".into());
        line(format!("target = {}", quote(c.target.name())));
        line(
            "# En blocs, 4 à 128 (le serveur n'envoie les joueurs que dans sa distance de suivi)."
                .into(),
        );
        line(format!("range = {}", c.range));
        line("# Secondes pendant lesquelles le panneau reste après avoir quitté le viseur.".into());
        line(format!("linger = {}", float(c.linger)));
        line("# Ce qu'il montre.".into());
        line(format!("game_mode = {}", c.game_mode));
        line(format!("ping = {}", c.ping));
        line(format!("health = {}", c.health));
        line(format!("equipment = {}", c.equipment));
        line("# Nom des objets en main, enchantements, durabilité.".into());
        line(format!("details = {}", c.details));
        line("# Posture et action en cours : sprint, accroupi, mange, bloque...".into());
        line(format!("state = {}", c.state));
        line(format!("mount = {}", c.mount));
        line(format!("coordinates = {}", c.coordinates));
        line("# Coups reçus, critiques, totems, boucliers, montures, changements d'arme.".into());
        line(format!("events = {}", c.events));

        out
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

fn quote(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn float(value: f32) -> String {
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_file_reads_back_as_the_same_config() {
        let mut config = Config::initial();
        config.language = Language::En;
        config.card.placement = Placement::Crosshair;
        config.card.layout = Layout::Panel;
        config.card.target = Targeting::Nearest;
        config.card.opacity = 0.5;
        config.card.offset_y = -12;
        config.card.events = false;
        assert_eq!(Config::parse(&config.to_toml()).unwrap(), config);
        assert!(config.to_toml().starts_with("# Players Info 1.0"));
    }

    #[test]
    fn a_missing_or_partial_file_is_on_with_defaults() {
        assert_eq!(Config::parse("").unwrap(), Config::initial());
        let read = Config::parse("[card]\nping = false\n").unwrap();
        assert!(read.enabled && !read.card.ping && read.card.health);
        assert!(!Config::parse("enabled = false\n").unwrap().enabled);
    }

    #[test]
    fn values_are_kept_in_range_and_typos_refused() {
        let read = Config::parse("[card]\nrange = 500\nopacity = 3.0\nscale = nan\n").unwrap();
        assert_eq!(read.card.range, 128);
        assert_eq!(read.card.opacity, 1.0);
        assert_eq!(read.card.scale, 1.0);
        assert!(Config::parse("[card]\nplacement = \"middle\"\n").is_err());
        assert!(Config::parse("[card]\npnig = true\n").is_err());
        assert!(Config::parse("[tag]\nhealth = true\n").is_err());
    }
}
