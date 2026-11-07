//! Facts about held and worn items the client can't look up: vanilla's durability, and what a
//! player is doing when they use one.

use crate::i18n::{self, Lang};
use horizon_client::mod_api::ItemStack;

/// `DataComponents.MAX_DAMAGE` of vanilla's damageable items; None for the rest.
pub fn max_durability(item: &str) -> Option<u32> {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    let fixed = match name {
        "bow" => 384,
        "crossbow" => 465,
        "trident" => 250,
        "shield" => 336,
        "elytra" => 432,
        "mace" => 500,
        "fishing_rod" | "flint_and_steel" | "brush" | "wolf_armor" => 64,
        "shears" => 238,
        "carrot_on_a_stick" => 25,
        "warped_fungus_on_a_stick" => 100,
        "turtle_helmet" => 275,
        _ => 0,
    };
    if fixed > 0 {
        return Some(fixed);
    }
    let (material, piece) = name.rsplit_once('_')?;
    // ArmorType.getDurability: the piece's unit times the material's multiplier.
    let unit = match piece {
        "helmet" => Some(11),
        "chestplate" => Some(16),
        "leggings" => Some(15),
        "boots" => Some(13),
        _ => None,
    };
    if let Some(unit) = unit {
        let multiplier = match material {
            "leather" => 5,
            "copper" => 11,
            "chainmail" | "iron" => 15,
            "golden" => 7,
            "diamond" => 33,
            "netherite" => 37,
            _ => return None,
        };
        return Some(unit * multiplier);
    }
    if !matches!(
        piece,
        "sword" | "pickaxe" | "axe" | "shovel" | "hoe" | "spear"
    ) {
        return None;
    }
    // ToolMaterial durability.
    Some(match material {
        "wooden" => 59,
        "stone" => 131,
        "copper" => 190,
        "iron" => 250,
        "golden" => 32,
        "diamond" => 1561,
        "netherite" => 2031,
        _ => return None,
    })
}

/// `Rarity`'s name colour: white, yellow, aqua or light purple. Enchanting raises common and
/// uncommon items to rare, and rare ones to epic (`ItemStack.getRarity`).
pub fn rarity_color(stack: &ItemStack) -> u32 {
    let name = stack.item.strip_prefix("minecraft:").unwrap_or(&stack.item);
    let base = match name {
        "totem_of_undying"
        | "elytra"
        | "experience_bottle"
        | "dragon_breath"
        | "nether_star"
        | "heart_of_the_sea"
        | "enchanted_book"
        | "creeper_head"
        | "zombie_head"
        | "skeleton_skull"
        | "wither_skeleton_skull"
        | "player_head"
        | "piglin_head"
        | "dragon_head"
        | "trial_key" => 1,
        "golden_apple" | "beacon" | "conduit" | "end_crystal" | "ominous_trial_key" | "trident" => {
            2
        }
        "enchanted_golden_apple" | "dragon_egg" | "mace" | "heavy_core" | "command_block" => 3,
        _ => 0,
    };
    let rarity = if stack.enchantments().is_empty() {
        base
    } else if base < 2 {
        2
    } else {
        3
    };
    match rarity {
        0 => 0xFFFFFF,
        1 => 0xFFFF55,
        2 => 0x55FFFF,
        _ => 0xFF55FF,
    }
}

/// What a player using `item` is doing: `Eating: Golden Apple`, `Blocking`.
pub fn use_label(lang: Lang, stack: &ItemStack) -> String {
    let name = stack.item.strip_prefix("minecraft:").unwrap_or(&stack.item);
    let action = match name {
        "shield" => return lang.pick("Bloque", "Blocking").to_owned(),
        "bow" => return lang.pick("Bande son arc", "Drawing a bow").to_owned(),
        "crossbow" => {
            return lang
                .pick("Charge son arbalète", "Loading a crossbow")
                .to_owned();
        }
        "trident" => {
            return lang
                .pick("Prépare un trident", "Readying a trident")
                .to_owned();
        }
        "spyglass" => return lang.pick("Longue-vue", "Spyglass").to_owned(),
        "goat_horn" => {
            return lang
                .pick("Souffle dans une corne", "Blowing a horn")
                .to_owned();
        }
        "brush" => return lang.pick("Brosse", "Brushing").to_owned(),
        n if drinks(n) => lang.pick("Boit", "Drinking"),
        n if FOODS.contains(&n) => lang.pick("Mange", "Eating"),
        _ => lang.pick("Utilise", "Using"),
    };
    format!(
        "{action}{}{}",
        lang.pick(" : ", ": "),
        i18n::item_name(stack)
    )
}

fn drinks(name: &str) -> bool {
    matches!(
        name,
        "potion" | "milk_bucket" | "honey_bottle" | "ominous_bottle"
    )
}

/// Vanilla's foods (items with `minecraft:food` and a consumable eat animation).
const FOODS: &[&str] = &[
    "apple",
    "golden_apple",
    "enchanted_golden_apple",
    "golden_carrot",
    "carrot",
    "bread",
    "cookie",
    "cake",
    "pumpkin_pie",
    "melon_slice",
    "glistering_melon_slice",
    "sweet_berries",
    "glow_berries",
    "chorus_fruit",
    "dried_kelp",
    "beetroot",
    "beetroot_soup",
    "mushroom_stew",
    "rabbit_stew",
    "suspicious_stew",
    "potato",
    "baked_potato",
    "poisonous_potato",
    "beef",
    "cooked_beef",
    "porkchop",
    "cooked_porkchop",
    "chicken",
    "cooked_chicken",
    "mutton",
    "cooked_mutton",
    "rabbit",
    "cooked_rabbit",
    "cod",
    "cooked_cod",
    "salmon",
    "cooked_salmon",
    "tropical_fish",
    "pufferfish",
    "rotten_flesh",
    "spider_eye",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, damage: u32) -> ItemStack {
        ItemStack {
            item: item.into(),
            count: 1,
            damage,
            extra: None,
        }
    }

    #[test]
    fn durability_matches_vanilla() {
        assert_eq!(max_durability("minecraft:diamond_chestplate"), Some(528));
        assert_eq!(max_durability("minecraft:netherite_boots"), Some(481));
        assert_eq!(max_durability("minecraft:iron_helmet"), Some(165));
        assert_eq!(max_durability("minecraft:leather_leggings"), Some(75));
        assert_eq!(max_durability("minecraft:netherite_sword"), Some(2031));
        assert_eq!(max_durability("minecraft:wooden_spear"), Some(59));
        assert_eq!(max_durability("minecraft:elytra"), Some(432));
        assert_eq!(max_durability("minecraft:golden_apple"), None);
        assert_eq!(max_durability("minecraft:stone"), None);
    }

    #[test]
    fn names_take_their_rarity_colour() {
        assert_eq!(rarity_color(&stack("minecraft:diamond_sword", 0)), 0xFFFFFF);
        assert_eq!(
            rarity_color(&stack("minecraft:totem_of_undying", 0)),
            0xFFFF55
        );
        assert_eq!(
            rarity_color(&stack("minecraft:enchanted_golden_apple", 0)),
            0xFF55FF
        );
        let mut enchanted = stack("minecraft:diamond_sword", 0);
        enchanted.extra = Some(Box::new(horizon_client::mod_api::StackData {
            enchantments: vec![("minecraft:sharpness".into(), 5)],
            custom_name: None,
        }));
        assert_eq!(rarity_color(&enchanted), 0x55FFFF);
    }

    #[test]
    fn using_an_item_says_what_it_is_for() {
        assert_eq!(
            use_label(Lang::En, &stack("minecraft:shield", 0)),
            "Blocking"
        );
        assert_eq!(
            use_label(Lang::Fr, &stack("minecraft:golden_apple", 0)),
            "Mange : Golden Apple"
        );
        assert_eq!(
            use_label(Lang::En, &stack("minecraft:potion", 0)),
            "Drinking: Potion"
        );
    }
}
