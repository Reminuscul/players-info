//! Words. The mod's own labels come in French and English; the names of things (items,
//! entities, enchantments) are the game's own, in whatever language it runs, through its
//! language files.

use crate::config::Language;
use horizon_client::lang;
use horizon_client::mod_api::ItemStack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    pub fn of(setting: Language) -> Self {
        match setting {
            Language::Fr => Lang::Fr,
            Language::En => Lang::En,
            Language::Auto if lang::code().starts_with("fr") => Lang::Fr,
            Language::Auto => Lang::En,
        }
    }

    /// The French or the English of a label.
    pub fn pick(self, fr: &'static str, en: &'static str) -> &'static str {
        match self {
            Lang::Fr => fr,
            Lang::En => en,
        }
    }

    pub fn game_mode(self, id: i32) -> Option<&'static str> {
        Some(match id {
            0 => self.pick("Survie", "Survival"),
            1 => self.pick("Créatif", "Creative"),
            2 => self.pick("Aventure", "Adventure"),
            3 => self.pick("Spectateur", "Spectator"),
            _ => return None,
        })
    }

    pub fn on_off(self, on: bool) -> &'static str {
        match (self, on) {
            (Lang::Fr, true) => "OUI",
            (Lang::Fr, false) => "NON",
            (Lang::En, true) => "ON",
            (Lang::En, false) => "OFF",
        }
    }
}

/// An item's name as the game shows it: its custom name, else its translated name.
pub fn item_name(stack: &ItemStack) -> String {
    if let Some(name) = stack.custom_name().filter(|n| !n.trim().is_empty()) {
        return name;
    }
    let (namespace, path) = split_id(&stack.item);
    translated(&[
        format!("item.{namespace}.{path}"),
        format!("block.{namespace}.{path}"),
    ])
    .unwrap_or_else(|| pretty(path))
}

/// An entity type's name (`minecraft:horse` → `Cheval` in French).
pub fn entity_name(kind: &str) -> String {
    let (namespace, path) = split_id(kind);
    translated(&[format!("entity.{namespace}.{path}")]).unwrap_or_else(|| pretty(path))
}

/// `Sharpness V`, as an item's tooltip writes it.
pub fn enchantment(id: &str, level: u32) -> String {
    let (namespace, path) = split_id(id);
    let name =
        translated(&[format!("enchantment.{namespace}.{path}")]).unwrap_or_else(|| pretty(path));
    if level == 1 && max_level_is_one(path) {
        return name;
    }
    let level = translated(&[format!("enchantment.level.{level}")]).unwrap_or_else(|| roman(level));
    format!("{name} {level}")
}

/// `Durability: 1170 / 1561`, as an advanced tooltip writes it (`item.durability`).
pub fn durability(left: u32, max: u32) -> String {
    let template = if lang::has("item.durability") {
        lang::get("item.durability")
    } else {
        "Durability: %s / %s".to_owned()
    };
    let mut out = String::new();
    let mut args = [left, max].into_iter();
    let mut rest = template.as_str();
    while let Some(at) = rest.find("%s") {
        out.push_str(&rest[..at]);
        out.push_str(&args.next().map_or_else(String::new, |n| n.to_string()));
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

pub fn is_curse(id: &str) -> bool {
    split_id(id).1.ends_with("_curse")
}

/// Enchantments that only have one level, written without it (`Mending`, not `Mending I`).
fn max_level_is_one(path: &str) -> bool {
    matches!(
        path,
        "mending"
            | "infinity"
            | "silk_touch"
            | "flame"
            | "channeling"
            | "multishot"
            | "aqua_affinity"
            | "binding_curse"
            | "vanishing_curse"
    )
}

fn translated(keys: &[String]) -> Option<String> {
    keys.iter().find(|k| lang::has(k)).map(|k| lang::get(k))
}

fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

/// `diamond_sword` → `Diamond Sword`, for ids the language has no name for.
pub fn pretty(path: &str) -> String {
    path.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect::<String>()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn roman(n: u32) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    const DIGITS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    let mut rest = n;
    for (value, digits) in DIGITS {
        while rest >= value {
            out.push_str(digits);
            rest -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_without_a_language_are_made_from_the_id() {
        assert_eq!(pretty("diamond_sword"), "Diamond Sword");
        assert_eq!(pretty("tnt"), "Tnt");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(10), "X");
        assert_eq!(roman(0), "0");
        assert_eq!(durability(1170, 1561), "Durability: 1170 / 1561");
        assert!(is_curse("minecraft:binding_curse"));
        assert!(!is_curse("minecraft:sharpness"));
    }

    #[test]
    fn labels_follow_the_choice() {
        assert_eq!(Lang::Fr.pick("Survie", "Survival"), "Survie");
        assert_eq!(Lang::En.game_mode(1), Some("Creative"));
        assert_eq!(Lang::Fr.game_mode(9), None);
        assert_eq!(Lang::of(Language::Fr), Lang::Fr);
    }
}
