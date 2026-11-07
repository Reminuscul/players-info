//! The card laid out around the player instead of in a box: the hearts over their name tag, game mode and ping either side of it, each armor piece left of the body at
//! the height of what it covers, what they hold and do on the right, what just happened under
//! their feet. It follows them on screen, smaller as they get farther, and is drawn with the
//! game's own sprites, font and item rendering.

use crate::anim::ease_out;
use crate::card::color::{DARK_GRAY, GRAY};
use crate::card::{self, Anim, LINE, Painted, Pen, Segments, Slot, Span, View, WORD_GAP};
use crate::events::LIFETIME;
use crate::i18n::Lang;
use horizon_client::hud_layout::{GuiDraw, PoseOp};

/// Where the player is on screen this frame, in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchors {
    /// The name tag's centre, half its width and its height, on screen.
    pub tag: [f32; 2],
    pub tag_half_width: f32,
    pub tag_height: f32,
    /// The body's box on screen: left, top, right, bottom.
    pub body: [f32; 4],
    /// Where the head, chest, legs and feet are on screen.
    pub parts: [f32; 4],
}

/// Space between the body (or the name tag) and what's beside it.
const GAP: f32 = 4.0;
const ITEM: f32 = 16.0;
/// The widest a line of text beside the body gets, at scale 1.
const TEXT_MAX: f32 = 170.0;
/// How far the sides start from their final place, against the body.
const SPREAD: f32 = 6.0;

/// Paints the view around the player at `anchors`, on a `screen` of GUI pixels.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    view: &View,
    anim: &mut Anim,
    anchors: &Anchors,
    lang: Lang,
    screen: [f32; 2],
    scale: f32,
    now: f64,
    dt: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Painted {
    let new_card = anim.presence < 0.02 || !anim.started;
    anim.started = true;
    let fresh = std::mem::take(&mut anim.fresh) || new_card;
    if new_card {
        anim.content = 1.0;
    }
    card::step_values(view, anim, fresh, now, dt);

    let alpha = anim.presence.clamp(0.0, 1.0) * anim.content;
    let spread = (1.0 - ease_out(anim.presence)) * SPREAD;
    let [left, top, right, bottom] = anchors.body;
    // Smaller with distance, down to 70%: the pieces stay readable and keep to the player.
    let k = ((bottom - top) / 90.0).clamp(0.7, 1.0) * scale;
    let mut p = Pen {
        draws: Vec::new(),
        alpha,
        measure,
    };
    let mut out = Painted::default();

    over_the_name(&mut p, view, anim, anchors, k, spread, now);
    beside_the_name(&mut p, view, anim, anchors, screen, k, spread);
    armor_on_the_left(&mut out, view, anchors, k, spread, alpha);
    held_on_the_right(
        &mut p, &mut out, view, anim, anchors, lang, screen, k, spread,
    );
    under_the_feet(
        &mut p,
        view,
        [(left + right) / 2.0, bottom],
        screen,
        k,
        spread,
    );

    out.draws = p.draws;
    out
}

/// Draws `f` at `at`, `k` times its size.
fn group(p: &mut Pen, at: [f32; 2], k: f32, f: impl FnOnce(&mut Pen)) {
    p.draws.push(GuiDraw::Push(vec![
        PoseOp::Translate(at[0], at[1]),
        PoseOp::Scale(k, k),
    ]));
    f(p);
    p.draws.push(GuiDraw::Pop);
}

/// The hearts, right over the name tag.
fn over_the_name(
    p: &mut Pen,
    view: &View,
    anim: &Anim,
    anchors: &Anchors,
    k: f32,
    spread: f32,
    now: f64,
) {
    let [x, y] = anchors.tag;
    let base = y - anchors.tag_height / 2.0 - 2.0 + spread / 2.0;
    if let Some(health) = view.health {
        let (containers, absorbing) = card::heart_counts(&health, &anim.hearts);
        let lines = card::heart_lines(containers + absorbing);
        let height = ((lines - 1) * card::heart_row_height(lines)) as f32 + 9.0;
        let width = ((containers + absorbing).clamp(1, 10) * 8 + 1) as f32;
        group(p, [x - width * k / 2.0, base - height * k], k, |p| {
            card::hearts(p, &anim.hearts, &health, view.hearts, 0.0, now);
        });
    }
}

/// The game mode left of the name tag, the ping right of it.
fn beside_the_name(
    p: &mut Pen,
    view: &View,
    anim: &Anim,
    anchors: &Anchors,
    screen: [f32; 2],
    k: f32,
    spread: f32,
) {
    let [x, y] = anchors.tag;
    let side = anchors.tag_half_width + GAP + spread;
    let line_top = y - 4.5 * k;
    if let Some(mode) = &view.game_mode {
        let w = p.width(mode) * k;
        group(p, [(x - side - w).max(4.0), line_top], k, |p| {
            p.text(0.0, 0.0, &Span::new(mode.as_str(), GRAY));
        });
    }
    if let Some(ms) = view.ping {
        let text = (ms > 0).then(|| format!("{:.0} ms", anim.ping.max(1.0)));
        let w = (12.0 + text.as_deref().map_or(0.0, |t| p.width(t))) * k;
        group(p, [(x + side).min(screen[0] - 4.0 - w), line_top], k, |p| {
            p.sprite(card::ping_sprite(ms), 0.0, 0.0, 10, 8);
            if let Some(text) = text {
                p.text(12.0, 0.0, &Span::new(text, GRAY));
            }
        });
    }
}

/// Each armor piece left of the body, level with what it covers; packed together when the
/// player is too far for that.
fn armor_on_the_left(
    out: &mut Painted,
    view: &View,
    anchors: &Anchors,
    k: f32,
    spread: f32,
    alpha: f32,
) {
    let size = ITEM * k;
    let mut last: Option<f32> = None;
    let x = anchors.body[0] - GAP - spread - size;
    for (i, (slot, _)) in view.items.iter().enumerate() {
        let Slot::Armor(n) = slot else {
            continue;
        };
        let wanted = anchors.parts[usize::from(*n).min(3)];
        let y = last.map_or(wanted, |l| wanted.max(l + size + k));
        last = Some(y);
        let drawn = size * ease_out(alpha.sqrt());
        if drawn >= 2.0 {
            out.items
                .push((i, [x + (size - drawn) / 2.0, y - drawn / 2.0], drawn));
        }
    }
}

/// Right of the body, from the chest down: the main hand's item and its tooltip lines, the
/// off hand's, then posture and actions, the mount, and where they are.
#[allow(clippy::too_many_arguments)]
fn held_on_the_right(
    p: &mut Pen,
    out: &mut Painted,
    view: &View,
    anim: &Anim,
    anchors: &Anchors,
    lang: Lang,
    screen: [f32; 2],
    k: f32,
    spread: f32,
) {
    let measure = p.measure;
    let width = |s: &Segments| s.iter().map(|span| measure(&span.text)).sum::<f32>();
    let cut = |lines: &[Segments]| -> Vec<Segments> {
        lines
            .iter()
            .map(|l| card::truncate(l, TEXT_MAX, measure))
            .collect()
    };

    let item = |slot: Slot| view.items.iter().position(|(s, _)| *s == slot);
    let main = cut(&view.main_details);
    let off = cut(&view.off_details);
    let words = card::wrap(&view.state, TEXT_MAX, measure);
    let mut tail: Vec<Segments> = Vec::new();
    if let Some(mount) = &view.mount {
        tail.push(mount.clone());
    }
    let mut place = vec![Span::new(card::away(lang, view.distance), GRAY)];
    if let Some(at) = view.position {
        place.push(Span::new(" · ", DARK_GRAY));
        place.push(Span::new(
            format!("{:.1} / {:.1} / {:.1}", at[0], at[1], at[2]),
            GRAY,
        ));
    }
    if anim.speed >= 0.4 {
        place.push(Span::new(
            format!("  {:.1} {}", anim.speed, lang.pick("blocs/s", "blocks/s")),
            DARK_GRAY,
        ));
    }
    tail.push(place);

    // The column's width, to keep it on screen.
    let indent = ITEM + 3.0;
    let text_width = main
        .iter()
        .chain(off.iter())
        .map(|l| indent + width(l))
        .chain(
            words
                .iter()
                .map(|l| width(l) + WORD_GAP * l.len().saturating_sub(1) as f32),
        )
        .chain(tail.iter().map(&width))
        .fold(0.0f32, f32::max);
    let x = (anchors.body[2] + GAP + spread)
        .min(screen[0] - 4.0 - text_width * k)
        .max(4.0);
    let mut y = (anchors.parts[1] - 8.0 * k).max(anchors.body[1]);

    // A held item: its icon, its lines beside it.
    let held = |p: &mut Pen, out: &mut Painted, slot: Slot, lines: &[Segments], y: &mut f32| {
        let index = item(slot);
        if index.is_none() && lines.is_empty() {
            return;
        }
        if let Some(i) = index {
            let pop = if slot == Slot::Main {
                1.0 + 0.2 * view.swing
            } else {
                1.0
            };
            let size = ITEM * k * ease_out(p.alpha.sqrt()) * pop;
            if size >= 2.0 {
                let center = [x + ITEM * k / 2.0, *y + ITEM * k / 2.0];
                out.items
                    .push((i, [center[0] - size / 2.0, center[1] - size / 2.0], size));
            }
        }
        // One line sits level with the icon's middle; more start at its top.
        let first = if lines.len() == 1 { 4.0 } else { 0.0 };
        group(p, [x + indent * k, *y], k, |p| {
            for (n, line) in lines.iter().enumerate() {
                p.segments(0.0, first + n as f32 * LINE, line);
            }
        });
        *y += (lines.len() as f32 * LINE).max(ITEM + 1.0) * k + 2.0 * k;
    };
    held(p, out, Slot::Main, &main, &mut y);
    held(p, out, Slot::Off, &off, &mut y);

    group(p, [x, y], k, |p| {
        let mut ly = 0.0;
        for line in &words {
            let mut lx = 0.0;
            for word in line {
                p.text(lx, ly, word);
                lx += p.width(&word.text) + WORD_GAP;
            }
            ly += LINE;
        }
        for line in &tail {
            p.segments(0.0, ly, line);
            ly += LINE;
        }
    });
}

/// What just happened, under the feet, newest first, each line fading in and out.
fn under_the_feet(p: &mut Pen, view: &View, at: [f32; 2], screen: [f32; 2], k: f32, spread: f32) {
    let base = p.alpha;
    let mut y = at[1] + GAP + spread / 2.0;
    for event in &view.events {
        let entering = ease_out((event.shown_for / 0.25) as f32);
        let leaving = ((LIFETIME - event.age) as f32).clamp(0.0, 1.0);
        p.alpha = base * entering * leaving;
        let w = p.width(&event.text) * k;
        let drop = (1.0 - entering) * -4.0 * k;
        let x = (at[0] - w / 2.0).clamp(4.0, (screen[0] - 4.0 - w).max(4.0));
        group(p, [x, y + drop], k, |p| {
            p.text(0.0, 0.0, &Span::new(event.text.as_str(), event.color));
        });
        y += LINE * k;
    }
    p.alpha = base;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{EventLine, Health};
    use horizon_client::mod_api::ItemStack;

    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    /// A player standing mid-screen, 90 pixels tall.
    fn anchors() -> Anchors {
        Anchors {
            tag: [240.0, 80.0],
            tag_half_width: 15.0,
            tag_height: 10.0,
            body: [225.0, 90.0, 255.0, 180.0],
            parts: [98.0, 120.0, 150.0, 175.0],
        }
    }

    fn stack(item: &str) -> ItemStack {
        ItemStack {
            item: item.into(),
            count: 1,
            damage: 0,
            extra: None,
        }
    }

    fn view() -> View {
        View {
            id: 7,
            name: "Alex".into(),
            game_mode: Some("Survival".into()),
            distance: 8.0,
            ping: Some(42),
            health: Some(Health {
                current: 20.0,
                max: Some(20.0),
                absorption: 0.0,
            }),
            items: vec![
                (Slot::Main, stack("minecraft:diamond_sword")),
                (Slot::Armor(0), stack("minecraft:diamond_helmet")),
                (Slot::Armor(1), stack("minecraft:diamond_chestplate")),
                (Slot::Armor(2), stack("minecraft:diamond_leggings")),
                (Slot::Armor(3), stack("minecraft:diamond_boots")),
            ],
            main_details: vec![vec![Span::new("Diamond Sword", 0xFFFFFF)]],
            events: vec![EventLine {
                seq: 1,
                text: "Hit by Steve".into(),
                color: 0xFF5555,
                age: 2.0,
                shown_for: 2.0,
            }],
            ..View::default()
        }
    }

    fn settled(view: &View, anchors: &Anchors) -> Painted {
        let mut anim = Anim::default();
        let mut now = 10.0;
        let mut last = Painted::default();
        for _ in 0..90 {
            now += 1.0 / 60.0;
            anim.show(true, 1.0 / 60.0);
            last = paint(
                view,
                &mut anim,
                anchors,
                Lang::En,
                [480.0, 270.0],
                1.0,
                now,
                1.0 / 60.0,
                &measure,
            );
        }
        last
    }

    /// Each text drawn, with where its group put it on screen.
    fn placed_texts(p: &Painted) -> Vec<(String, [f32; 2])> {
        let mut at = [0.0, 0.0];
        let mut k = 1.0;
        let mut out = Vec::new();
        for d in &p.draws {
            match d {
                GuiDraw::Push(ops) => {
                    for op in ops {
                        match op {
                            PoseOp::Translate(x, y) => at = [*x, *y],
                            PoseOp::Scale(s, _) => k = *s,
                        }
                    }
                }
                GuiDraw::Text { text, x, y, .. } => {
                    out.push((text.plain(), [at[0] + *x as f32 * k, at[1] + *y as f32 * k]))
                }
                _ => {}
            }
        }
        out
    }

    fn sprite_count(p: &Painted, name: &str) -> usize {
        p.draws
            .iter()
            .filter(|d| matches!(d, GuiDraw::Sprite { sprite, .. } if sprite == name))
            .count()
    }

    #[test]
    fn hearts_alone_sit_over_the_name_tag() {
        let p = settled(&view(), &anchors());
        assert_eq!(sprite_count(&p, "hud/heart/full"), 10);
        assert_eq!(sprite_count(&p, "hud/armor_full"), 0, "no armor bar");
        // Above the tag's top (75).
        let tops: Vec<f32> = p
            .draws
            .iter()
            .filter_map(|d| match d {
                GuiDraw::Push(ops) => match ops[0] {
                    PoseOp::Translate(_, y) => Some(y),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        // The hearts end 2 pixels over the tag: 75 - 2 - 9.
        assert_eq!(tops[0], 64.0, "{tops:?}");
    }

    #[test]
    fn game_mode_and_ping_flank_the_name_tag() {
        let p = settled(&view(), &anchors());
        let texts = placed_texts(&p);
        let mode = texts.iter().find(|t| t.0 == "Survival").unwrap();
        let ping = texts.iter().find(|t| t.0 == "42 ms").unwrap();
        // Ends 4 pixels left of the tag (225), starts after the bars right of it (259).
        assert_eq!(mode.1[0] + measure("Survival"), 221.0);
        assert_eq!(ping.1[0], 240.0 + 15.0 + 4.0 + 12.0);
        assert_eq!(mode.1[1], 80.0 - 4.5);
    }

    #[test]
    fn armor_pieces_line_up_with_what_they_cover() {
        let p = settled(&view(), &anchors());
        let armor: Vec<[f32; 2]> = p.items.iter().filter(|i| i.0 > 0).map(|i| i.1).collect();
        // Left of the body, centred on the head, chest, legs and feet.
        assert!(armor.iter().all(|at| at[0] == 225.0 - 4.0 - 16.0));
        let centres: Vec<f32> = armor.iter().map(|at| at[1] + 8.0).collect();
        assert_eq!(centres, [98.0, 120.0, 150.0, 175.0]);
    }

    #[test]
    fn far_away_the_pieces_pack_and_shrink() {
        let mut far = anchors();
        far.body = [235.0, 120.0, 245.0, 150.0];
        far.parts = [122.0, 130.0, 140.0, 148.0];
        let p = settled(&view(), &far);
        let armor: Vec<&(usize, [f32; 2], f32)> = p.items.iter().filter(|i| i.0 > 0).collect();
        assert!(armor.iter().all(|i| (i.2 - 16.0 * 0.7).abs() < 1e-4));
        for pair in armor.windows(2) {
            assert!(pair[1].1[1] - pair[0].1[1] >= 16.0 * 0.7);
        }
    }

    #[test]
    fn the_held_item_and_events_go_right_and_under() {
        let p = settled(&view(), &anchors());
        let main = *p.items.iter().find(|i| i.0 == 0).unwrap();
        assert_eq!(main.1[0], 255.0 + 4.0);
        let texts = placed_texts(&p);
        let name = texts.iter().find(|t| t.0 == "Diamond Sword").unwrap();
        assert_eq!(name.1[0], 255.0 + 4.0 + 19.0);
        let hit = texts.iter().find(|t| t.0 == "Hit by Steve").unwrap();
        assert_eq!(hit.1[0] + measure("Hit by Steve") / 2.0, 240.0);
        assert!(hit.1[1] > 180.0);
        assert!(texts.iter().any(|t| t.0 == "8.0 blocks away"));
    }

    #[test]
    fn the_right_side_stays_on_screen() {
        let mut edge = anchors();
        edge.body = [440.0, 90.0, 470.0, 180.0];
        let p = settled(&view(), &edge);
        let texts = placed_texts(&p);
        for (text, at) in &texts {
            assert!(at[0] + measure(text) <= 480.0, "{text} at {at:?}");
        }
    }
}
