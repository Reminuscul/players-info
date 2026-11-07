//! The card: a panel on the HUD about one player, drawn with the game's own GUI. It is the
//! vanilla tooltip box; the hearts are the health bar's, blinking, lagging and
//! shaking as the player's own do; the items sit in the hotbar's slots, the main hand selected,
//! the off hand in its own slot; the ping is the player list's bars; the text uses the game's
//! colours (`§7` grey for what describes, `§b` aqua for an enchanted item...).
//!
//! It slides in a few pixels and fades when a player is targeted, fades out when none is, and
//! each row opens and closes on its own as what it shows comes and goes. Everything here is
//! layout over a `View` the caller fills in, so it is tested without the game.

use crate::anim::{approach, ease_out, opaque};
use crate::config::Placement;
use crate::events::LIFETIME;
use crate::i18n::Lang;
use horizon_client::hud_layout::{Blend, GuiDraw, PoseOp};
use horizon_client::mod_api::{ItemStack, Text};
use std::borrow::Cow;

/// The game's text colours (`ChatFormatting`).
pub mod color {
    pub const WHITE: u32 = 0xFFFFFF;
    pub const GRAY: u32 = 0xAAAAAA;
    pub const DARK_GRAY: u32 = 0x555555;
    pub const YELLOW: u32 = 0xFFFF55;
    pub const GOLD: u32 = 0xFFAA00;
    pub const AQUA: u32 = 0x55FFFF;
    pub const GREEN: u32 = 0x55FF55;
    pub const RED: u32 = 0xFF5555;
    pub const LIGHT_PURPLE: u32 = 0xFF55FF;
}
use color::*;

/// A line of text, in GUI pixels at scale 1 (the font is 9 high).
pub(crate) const LINE: f32 = 10.0;
/// Space above a new section, as the tooltip's gap under its title.
const SECTION: f32 = 3.0;
const MIN_WIDTH: f32 = 140.0;
const MAX_WIDTH: f32 = 220.0;
/// Space between two state words.
pub(crate) const WORD_GAP: f32 = 7.0;
/// How far the card slides as it comes in.
const SLIDE: f32 = 12.0;

/// A run of text in one colour (`0xRRGGBB`), italic for a renamed item as the game shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: u32,
    pub italic: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, color: u32) -> Self {
        Self {
            text: text.into(),
            color,
            italic: false,
        }
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

pub type Segments = Vec<Span>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: Option<f32>,
    pub absorption: f32,
}

/// Which hearts the health bar draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeartStyle {
    /// A hardcore world's hearts.
    pub hardcore: bool,
    /// Fully frozen in powder snow: the frozen hearts.
    pub frozen: bool,
}

/// Where an item goes: the hotbar slot (main hand), the off-hand slot, or an armor slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Main,
    Off,
    /// 0 head to 3 feet.
    Armor(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventLine {
    pub seq: u64,
    pub text: String,
    pub color: u32,
    /// Seconds since it last happened, and since its line appeared.
    pub age: f64,
    pub shown_for: f64,
}

/// What the card shows about its player this frame, already in words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub id: u32,
    pub name: String,
    /// The skin texture for the face; None draws none.
    pub skin: Option<String>,
    /// The game mode's name, and how far away the player is, in blocks.
    pub game_mode: Option<String>,
    pub distance: f64,
    pub ping: Option<i32>,
    pub health: Option<Health>,
    pub hearts: HeartStyle,
    pub items: Vec<(Slot, ItemStack)>,
    /// The main hand's item as its tooltip starts (name, enchantments, durability), and the
    /// off hand's name.
    pub main_details: Vec<Segments>,
    pub off_details: Vec<Segments>,
    pub state: Segments,
    pub mount: Option<Segments>,
    pub position: Option<[f64; 3]>,
    /// Blocks per second.
    pub speed: f32,
    pub events: Vec<EventLine>,
    /// 1 just as the main arm swings, easing to 0.
    pub swing: f32,
}

/// What a frame of the card paints: nine-sliced sprites (the tooltip box), then GUI calls,
/// then items at GUI pixel positions.
#[derive(Debug, Default, PartialEq)]
pub struct Painted {
    pub boxes: Vec<(&'static str, [f32; 4], u32)>,
    pub draws: Vec<GuiDraw>,
    /// Index into `View::items`, top-left corner, size.
    pub items: Vec<(usize, [f32; 2], f32)>,
}

/// How the card looks, from the config.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub placement: Placement,
    pub offset: [f32; 2],
    pub scale: f32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RowKey {
    Header,
    Hearts,
    Items,
    Detail(usize),
    State(usize),
    Mount,
    Coordinates,
    Event(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RowAnim {
    key: RowKey,
    /// 0 closed to 1 open.
    open: f32,
    height: f32,
}

/// `Gui.extractPlayerHealth`'s state for one player: the health last seen, the health still
/// shown as blinking hearts for a second after a hit, and until when the hearts blink.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HeartsAnim {
    last: i32,
    display: i32,
    last_change: f64,
    blink_until: f64,
}

/// The card's motion from frame to frame.
#[derive(Debug, Default)]
pub struct Anim {
    /// 0 off screen to 1 in place.
    pub presence: f32,
    /// Fades the content alone when the card changes player.
    pub content: f32,
    width: f32,
    rows: Vec<RowAnim>,
    pub(crate) hearts: HeartsAnim,
    pub(crate) ping: f32,
    pub(crate) speed: f32,
    /// The next frame starts over: a new player.
    pub(crate) fresh: bool,
    /// A frame was painted since the card came on screen.
    pub(crate) started: bool,
}

impl Anim {
    /// The next frame is about another player: values jump to theirs instead of easing.
    pub fn restart(&mut self) {
        self.fresh = true;
    }

    /// Moves `presence` toward shown (1) or hidden (0).
    pub fn show(&mut self, shown: bool, dt: f32) {
        let (target, rate) = if shown { (1.0, 12.0) } else { (0.0, 10.0) };
        self.presence = approach(self.presence, target, rate, dt);
    }

    /// Fades the content in (1) or out (0), as the card changes player.
    pub fn content_toward(&mut self, target: f32, dt: f32) {
        self.content = approach(self.content, target, 16.0, dt);
    }
}

/// One row before layout.
struct Row<'a> {
    key: RowKey,
    /// Space above its content, included in its height.
    space: f32,
    height: f32,
    width: f32,
    content: Content<'a>,
}

enum Content<'a> {
    Header {
        ping: Option<(String, &'static str)>,
        subtitle: Segments,
    },
    Hearts(Health),
    Items,
    Text(Segments),
    Words(Segments),
    /// Text, and a last part against the right edge.
    Split(Segments, Option<Span>),
    Event(&'a EventLine, String),
}

/// Paints the card for `view` on a `screen` of GUI pixels, at `now` seconds, `dt` after the
/// last frame.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    view: &View,
    anim: &mut Anim,
    style: &Style,
    lang: Lang,
    screen: [f32; 2],
    now: f64,
    dt: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Painted {
    // A card coming on screen starts complete; one changing player starts over, its content
    // first faded out by the caller.
    let new_card = anim.presence < 0.02 || !anim.started;
    anim.started = true;
    let fresh = std::mem::take(&mut anim.fresh) || new_card;
    if new_card {
        anim.content = 1.0;
    }
    step_values(view, anim, fresh, now, dt);

    let rows = rows(view, anim, lang, measure);
    let natural = rows.iter().map(|r| r.width).fold(0.0f32, f32::max);
    let width = natural.clamp(MIN_WIDTH, MAX_WIDTH);
    anim.width = if fresh {
        width
    } else {
        approach(anim.width, width, 14.0, dt)
    };
    let w = anim.width.round();
    let layout = open_rows(&mut anim.rows, &rows, fresh, dt);
    let height = layout
        .iter()
        .map(|r| r.height * r.open)
        .sum::<f32>()
        .max(LINE);

    let s = style.scale;
    let alpha = anim.presence.clamp(0.0, 1.0);
    let [x, y] = place(style, screen, [w * s, height * s], ease_out(anim.presence));
    let mut out = Painted::default();

    // The tooltip box, as `TooltipRenderUtil` draws it: its sprites carry 9 pixels of margin,
    // then 3 of box around the content.
    let rect = [x - 12.0, y - 12.0, w * s + 24.0, height * s + 24.0];
    out.boxes.push((
        "gui/sprites/tooltip/background",
        rect,
        opaque(0xFFFFFF, alpha * style.opacity),
    ));
    out.boxes
        .push(("gui/sprites/tooltip/frame", rect, opaque(0xFFFFFF, alpha)));

    let mut p = Pen {
        draws: vec![GuiDraw::Push(vec![
            PoseOp::Translate(x, y),
            PoseOp::Scale(s, s),
        ])],
        alpha,
        measure,
    };
    let content = alpha * anim.content;
    let mut top = 0.0;
    for slot in &layout {
        let open = slot.open;
        if let Some(row) = rows.iter().find(|r| r.key == slot.key)
            && open > 0.02
        {
            // A row's content shows once it has nearly its room, so lines never overlap.
            let reveal = ((open - 0.8) / 0.2).clamp(0.0, 1.0);
            p.alpha = content * reveal * reveal * (3.0 - 2.0 * reveal);
            let at = Place {
                origin: [x, y],
                scale: s,
                top: top + row.space,
                width: w,
            };
            draw_row(&mut p, &mut out, view, anim, row, &at, now);
        }
        top += slot.height * open;
    }
    p.draws.push(GuiDraw::Pop);
    out.draws = p.draws;
    out
}

/// Eases the numbers toward the view's, or jumps there for a new player; runs the hearts as
/// the health bar does.
pub(crate) fn step_values(view: &View, anim: &mut Anim, fresh: bool, now: f64, dt: f32) {
    if let Some(h) = view.health {
        let current = h.current.ceil() as i32;
        let hearts = &mut anim.hearts;
        let tick = now * 20.0;
        if fresh {
            *hearts = HeartsAnim {
                last: current,
                display: current,
                last_change: now,
                blink_until: 0.0,
            };
        } else {
            // `Gui.extractPlayerHealth`: a loss blinks for 20 ticks, a gain for 10; the old
            // health stays drawn under the blink for a second.
            if current < hearts.last {
                hearts.last_change = now;
                hearts.blink_until = tick + 20.0;
            } else if current > hearts.last {
                hearts.last_change = now;
                hearts.blink_until = tick + 10.0;
            }
            if now - hearts.last_change > 1.0 {
                hearts.display = current;
                hearts.last_change = now;
            }
            hearts.last = current;
        }
    }
    let ping = view.ping.unwrap_or(0) as f32;
    anim.ping = if fresh {
        ping
    } else {
        approach(anim.ping, ping, 3.0, dt)
    };
    anim.speed = if fresh {
        view.speed
    } else {
        approach(anim.speed, view.speed, 5.0, dt)
    };
}

fn rows<'a>(
    view: &'a View,
    anim: &Anim,
    lang: Lang,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<Row<'a>> {
    let width = |segments: &Segments| segments.iter().map(|s| measure(&s.text)).sum::<f32>();
    let section = |rows: &Vec<Row>| if rows.is_empty() { 0.0 } else { SECTION };
    let mut rows = Vec::new();

    // Face, name and ping bars; game mode and distance under the name.
    let ping = view.ping.map(|ms| {
        let text = if ms > 0 {
            format!("{:.0} ms", anim.ping.max(1.0))
        } else {
            String::new()
        };
        (text, ping_sprite(ms))
    });
    let face = if view.skin.is_some() { 20.0 } else { 0.0 };
    let top_line =
        measure(&view.name) + ping.as_ref().map_or(0.0, |(t, _)| 10.0 + 14.0 + measure(t));
    let mut subtitle = Vec::new();
    if let Some(mode) = &view.game_mode {
        subtitle.push(Span::new(mode.as_str(), GRAY));
        subtitle.push(Span::new(" · ", DARK_GRAY));
    }
    subtitle.push(Span::new(away(lang, view.distance), GRAY));
    rows.push(Row {
        key: RowKey::Header,
        space: 0.0,
        height: 19.0,
        width: face + top_line.max(width(&subtitle)),
        content: Content::Header { ping, subtitle },
    });

    if let Some(h) = view.health {
        let (containers, absorbing) = heart_counts(&h, &anim.hearts);
        let lines = heart_lines(containers + absorbing);
        let space = section(&rows);
        rows.push(Row {
            key: RowKey::Hearts,
            space,
            height: space + ((lines - 1) * heart_row_height(lines)) as f32 + 10.0,
            width: ((containers + absorbing).clamp(1, 10) * 8 + 1) as f32,
            content: Content::Hearts(h),
        });
    }
    if !view.items.is_empty() {
        let space = section(&rows);
        rows.push(Row {
            key: RowKey::Items,
            space,
            height: space + 25.0,
            width: strips(&view.items).width,
            content: Content::Items,
        });
    }

    let off = view.off_details.iter().map(|line| {
        let mut prefixed = vec![Span::new(lang.pick("Seconde main : ", "Off hand: "), GRAY)];
        prefixed.extend(line.iter().cloned());
        prefixed
    });
    let details: Vec<Segments> = view.main_details.iter().cloned().chain(off).collect();
    for (i, details) in details.iter().enumerate() {
        let details = truncate(details, MAX_WIDTH, measure);
        let space = if i == 0 { section(&rows) } else { 0.0 };
        rows.push(Row {
            key: RowKey::Detail(i),
            space,
            height: space + LINE,
            width: width(&details),
            content: Content::Text(details),
        });
    }
    for (i, words) in wrap(&view.state, MAX_WIDTH, measure)
        .into_iter()
        .enumerate()
    {
        let w = width(&words) + WORD_GAP * words.len().saturating_sub(1) as f32;
        let space = if i == 0 { section(&rows) } else { 0.0 };
        rows.push(Row {
            key: RowKey::State(i),
            space,
            height: space + LINE,
            width: w,
            content: Content::Words(words),
        });
    }
    if let Some(mount) = &view.mount {
        rows.push(Row {
            key: RowKey::Mount,
            space: 0.0,
            height: LINE,
            width: width(mount),
            content: Content::Text(mount.clone()),
        });
    }
    if let Some(at) = view.position {
        let segments = vec![
            Span::new(lang.pick("XYZ : ", "XYZ: "), GRAY),
            Span::new(format!("{:.1} / {:.1} / {:.1}", at[0], at[1], at[2]), WHITE),
        ];
        let speed = (anim.speed >= 0.4).then(|| {
            Span::new(
                format!("{:.1} {}", anim.speed, lang.pick("blocs/s", "blocks/s")),
                DARK_GRAY,
            )
        });
        let w = width(&segments) + speed.as_ref().map_or(0.0, |s| measure(&s.text) + 8.0);
        rows.push(Row {
            key: RowKey::Coordinates,
            space: 0.0,
            height: LINE,
            width: w,
            content: Content::Split(segments, speed),
        });
    }
    for (i, event) in view.events.iter().enumerate() {
        let age = if event.age < 1.0 {
            lang.pick("à l'instant", "now").to_owned()
        } else {
            format!("{:.0} s", event.age.floor())
        };
        let space = if i == 0 { section(&rows) + 1.0 } else { 0.0 };
        rows.push(Row {
            key: RowKey::Event(event.seq),
            space,
            height: space + LINE,
            width: measure(&event.text) + 10.0 + measure(&age),
            content: Content::Event(event, age),
        });
    }
    rows
}

/// The rows to lay out, in order: today's, and those still closing where they were.
fn open_rows(previous: &mut Vec<RowAnim>, rows: &[Row], fresh: bool, dt: f32) -> Vec<RowAnim> {
    let mut next: Vec<RowAnim> = rows
        .iter()
        .map(|r| {
            let open = match previous.iter().find(|p| p.key == r.key) {
                _ if fresh => 1.0,
                Some(p) => approach(p.open, 1.0, 13.0, dt),
                None => approach(0.0, 1.0, 13.0, dt),
            };
            RowAnim {
                key: r.key,
                open,
                height: r.height,
            }
        })
        .collect();
    if !fresh {
        // A row that went away closes in its place: after the row it followed.
        for (i, old) in previous.iter().enumerate() {
            if next.iter().any(|n| n.key == old.key) {
                continue;
            }
            let open = approach(old.open, 0.0, 13.0, dt);
            if open <= 0.01 {
                continue;
            }
            let after = previous[..i]
                .iter()
                .rev()
                .find_map(|p| next.iter().position(|n| n.key == p.key));
            next.insert(
                after.map_or(0, |a| a + 1),
                RowAnim {
                    key: old.key,
                    open,
                    height: old.height,
                },
            );
        }
    }
    *previous = next.clone();
    next
}

/// Where the content's top-left corner goes, sliding in from its edge as `shown` goes to 1.
fn place(style: &Style, screen: [f32; 2], size: [f32; 2], shown: f32) -> [f32; 2] {
    let [sw, sh] = screen;
    let [w, h] = size;
    let slide = (1.0 - shown) * SLIDE;
    let [ox, oy] = style.offset;
    // The box reaches 4 pixels around the content; that stays on screen too.
    let margin = 8.0;
    let (x, y) = match style.placement {
        Placement::Right => (sw - w - margin + slide, sh * 0.3),
        Placement::Left => (margin - slide, sh * 0.3),
        Placement::Top => ((sw - w) / 2.0, 28.0 - slide),
        Placement::Crosshair => (sw / 2.0 + 16.0 + slide, sh / 2.0 - 10.0),
    };
    let x = (x + ox).clamp(margin - slide, (sw - w - margin).max(margin) + slide);
    let y = (y + oy).clamp(margin, (sh - h - margin).max(margin));
    [x.round(), y.round()]
}

/// Where a row is drawn.
struct Place {
    origin: [f32; 2],
    scale: f32,
    /// The row's content top, within the card.
    top: f32,
    width: f32,
}

fn draw_row(
    p: &mut Pen,
    out: &mut Painted,
    view: &View,
    anim: &Anim,
    row: &Row,
    at: &Place,
    now: f64,
) {
    let (top, right) = (at.top, at.width);
    match &row.content {
        Content::Header { ping, subtitle } => {
            let x = match &view.skin {
                Some(skin) => {
                    p.face(skin, 0.0, top + 1.0);
                    20.0
                }
                None => 0.0,
            };
            p.text(x, top, &Span::new(view.name.as_str(), WHITE));
            p.segments(x, top + 10.0, subtitle);
            if let Some((text, bars)) = ping {
                p.sprite(bars, right - 10.0, top, 10, 8);
                let tw = p.width(text);
                p.text(right - 12.0 - tw, top, &Span::new(text.as_str(), GRAY));
            }
        }
        Content::Hearts(health) => hearts(p, &anim.hearts, health, view.hearts, top, now),
        Content::Items => {
            let layout = strips(&view.items);
            if layout.off {
                p.sprite("hud/hotbar_offhand_left", 0.0, top, 29, 24);
            }
            // The main hand: a one-slot hotbar, selected.
            p.strip(layout.main_x, top + 1.0, 1);
            p.sprite("hud/hotbar_selection", layout.main_x - 1.0, top, 24, 23);
            if layout.armor {
                p.strip(layout.armor_x, top + 1.0, 4);
            }
            for (i, (slot, _)) in view.items.iter().enumerate() {
                let lx = match slot {
                    Slot::Main => layout.main_x + 3.0,
                    Slot::Off => 3.0,
                    Slot::Armor(n) => layout.armor_x + 3.0 + 20.0 * f32::from(*n),
                };
                let ly = top + 4.0;
                // Items appear as their row does, and the main hand pops on a swing as an
                // item picked up does in the hotbar.
                let pop = if *slot == Slot::Main {
                    1.0 + 0.2 * view.swing
                } else {
                    1.0
                };
                let size = 16.0 * at.scale * ease_out(p.alpha.sqrt()) * pop;
                if size < 2.0 {
                    continue;
                }
                let center = [
                    at.origin[0] + (lx + 8.0) * at.scale,
                    at.origin[1] + (ly + 8.0) * at.scale,
                ];
                out.items
                    .push((i, [center[0] - size / 2.0, center[1] - size / 2.0], size));
            }
        }
        Content::Text(segments) => p.segments(0.0, top, segments),
        Content::Words(words) => {
            let mut x = 0.0;
            for word in words {
                p.text(x, top, word);
                x += p.width(&word.text) + WORD_GAP;
            }
        }
        Content::Split(segments, end) => {
            p.segments(0.0, top, segments);
            if let Some(end) = end {
                let tw = p.width(&end.text);
                p.text(right - tw, top, end);
            }
        }
        Content::Event(event, age) => {
            let base = p.alpha;
            let entering = ease_out((event.shown_for / 0.25) as f32);
            let leaving = ((LIFETIME - event.age) as f32).clamp(0.0, 1.0);
            p.alpha = base * entering * leaving;
            let slide = (1.0 - entering) * -6.0;
            p.text(slide, top, &Span::new(event.text.as_str(), event.color));
            let tw = p.width(age);
            p.text(right - tw, top, &Span::new(age.as_str(), DARK_GRAY));
            p.alpha = base;
        }
    }
}

/// How far away, in the mod's language: `à 8.0 blocs`, `8.0 blocks away`.
pub(crate) fn away(lang: Lang, blocks: f64) -> String {
    match lang {
        Lang::Fr => format!("à {blocks:.1} blocs"),
        Lang::En => format!("{blocks:.1} blocks away"),
    }
}

/// Heart containers for the maximum health and for the absorption, as the health bar counts
/// them.
pub(crate) fn heart_counts(h: &Health, anim: &HeartsAnim) -> (i32, i32) {
    let current = h.current.ceil() as i32;
    let max = h.max.unwrap_or(20.0).max(anim.display.max(current) as f32);
    let containers = (max / 2.0).ceil() as i32;
    let absorbing = (h.absorption.ceil() / 2.0).ceil() as i32;
    (containers, absorbing)
}

pub(crate) fn heart_lines(hearts: i32) -> i32 {
    ((hearts + 9) / 10).max(1)
}

/// Rows of hearts close up as there are more of them (`Gui.extractPlayerHealth`).
pub(crate) fn heart_row_height(lines: i32) -> i32 {
    (10 - (lines - 2)).max(3)
}

/// The hearts, as `Gui.extractPlayerHealth` lays them out for the player's own health.
pub(crate) fn hearts(
    p: &mut Pen,
    anim: &HeartsAnim,
    h: &Health,
    style: HeartStyle,
    top: f32,
    now: f64,
) {
    let tick = (now * 20.0).floor();
    let blink = anim.blink_until > tick && ((anim.blink_until - tick) as i64 / 3) % 2 == 1;
    let current = h.current.ceil() as i32;
    let absorption = h.absorption.ceil() as i32;
    let old = anim.display;
    let (containers, absorbing) = heart_counts(h, anim);
    let lines = heart_lines(containers + absorbing);
    let row_height = heart_row_height(lines);
    let base = top + ((lines - 1) * row_height) as f32;
    let low = current + absorption <= 4;
    let kind = if style.frozen {
        Heart::Frozen
    } else {
        Heart::Normal
    };
    for c in (0..containers + absorbing).rev() {
        let (line, column) = (c / 10, c % 10);
        let x = (column * 8) as f32;
        let mut y = base - (line * row_height) as f32;
        if low {
            // The low-health shake: a pixel down at random, each tick.
            y += (jitter(tick as i64, c) % 2) as f32;
        }
        let container = heart(Heart::Container, style.hardcore, false, blink);
        p.sprite(container, x, y, 9, 9);
        let halves = c * 2;
        if c >= containers {
            let ah = halves - containers * 2;
            if ah < absorption {
                let name = heart(
                    Heart::Absorbing,
                    style.hardcore,
                    ah + 1 == absorption,
                    false,
                );
                p.sprite(name, x, y, 9, 9);
            }
        }
        if blink && halves < old {
            p.sprite(
                heart(kind, style.hardcore, halves + 1 == old, true),
                x,
                y,
                9,
                9,
            );
        }
        if halves < current {
            p.sprite(
                heart(kind, style.hardcore, halves + 1 == current, false),
                x,
                y,
                9,
                9,
            );
        }
    }
}

/// Stands in for the game's random draw: steady within a tick, new the next.
fn jitter(tick: i64, n: i32) -> u64 {
    let mut v =
        (tick as u64).wrapping_mul(312_871) ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    v ^= v >> 33;
    v = v.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    v ^ (v >> 33)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Heart {
    Container,
    Normal,
    Frozen,
    Absorbing,
}

/// `Gui.HeartType`'s sprites.
fn heart(kind: Heart, hardcore: bool, half: bool, blink: bool) -> &'static str {
    let table: [&'static str; 8] = match kind {
        Heart::Container => [
            "hud/heart/container",
            "hud/heart/container_blinking",
            "hud/heart/container",
            "hud/heart/container_blinking",
            "hud/heart/container_hardcore",
            "hud/heart/container_hardcore_blinking",
            "hud/heart/container_hardcore",
            "hud/heart/container_hardcore_blinking",
        ],
        Heart::Absorbing => [
            "hud/heart/absorbing_full",
            "hud/heart/absorbing_full_blinking",
            "hud/heart/absorbing_half",
            "hud/heart/absorbing_half_blinking",
            "hud/heart/absorbing_hardcore_full",
            "hud/heart/absorbing_hardcore_full_blinking",
            "hud/heart/absorbing_hardcore_half",
            "hud/heart/absorbing_hardcore_half_blinking",
        ],
        Heart::Normal => [
            "hud/heart/full",
            "hud/heart/full_blinking",
            "hud/heart/half",
            "hud/heart/half_blinking",
            "hud/heart/hardcore_full",
            "hud/heart/hardcore_full_blinking",
            "hud/heart/hardcore_half",
            "hud/heart/hardcore_half_blinking",
        ],
        Heart::Frozen => [
            "hud/heart/frozen_full",
            "hud/heart/frozen_full_blinking",
            "hud/heart/frozen_half",
            "hud/heart/frozen_half_blinking",
            "hud/heart/frozen_hardcore_full",
            "hud/heart/frozen_hardcore_full_blinking",
            "hud/heart/frozen_hardcore_half",
            "hud/heart/frozen_hardcore_half_blinking",
        ],
    };
    table[usize::from(hardcore) * 4 + usize::from(half) * 2 + usize::from(blink)]
}

/// `PlayerTabOverlay.extractPingIcon`'s bars.
pub fn ping_sprite(ms: i32) -> &'static str {
    match ms {
        i32::MIN..=-1 => "icon/ping_unknown",
        0..150 => "icon/ping_5",
        150..300 => "icon/ping_4",
        300..600 => "icon/ping_3",
        600..1000 => "icon/ping_2",
        _ => "icon/ping_1",
    }
}

/// Where the slots go: the off hand's own slot, the main hand's, then the armor's four.
struct Strips {
    off: bool,
    armor: bool,
    main_x: f32,
    armor_x: f32,
    width: f32,
}

fn strips(items: &[(Slot, ItemStack)]) -> Strips {
    let off = items.iter().any(|(s, _)| *s == Slot::Off);
    let armor = items.iter().any(|(s, _)| matches!(s, Slot::Armor(_)));
    // The off-hand slot sprite ends where the hotbar starts, as in the game.
    let main_x = if off { 29.0 } else { 1.0 };
    let armor_x = main_x + 22.0 + 6.0;
    let width = if armor { armor_x + 82.0 } else { main_x + 23.0 };
    Strips {
        off,
        armor,
        main_x,
        armor_x,
        width,
    }
}

/// Segments cut to `max` pixels, the last one ending in `…`.
pub(crate) fn truncate(segments: &Segments, max: f32, measure: &dyn Fn(&str) -> f32) -> Segments {
    let mut out = Vec::new();
    let mut used = 0.0;
    for span in segments {
        let w = measure(&span.text);
        if used + w <= max {
            out.push(span.clone());
            used += w;
            continue;
        }
        let ellipsis = measure("…");
        let mut cut = String::new();
        for c in span.text.chars() {
            let mut next = cut.clone();
            next.push(c);
            if used + measure(&next) + ellipsis > max {
                break;
            }
            cut = next;
        }
        let cut = cut.trim_end().to_owned();
        if !cut.is_empty() {
            out.push(Span {
                text: format!("{cut}…"),
                ..span.clone()
            });
        } else if let Some(last) = out.last_mut() {
            last.text.push('…');
        }
        break;
    }
    out
}

/// Words in lines of at most `max` pixels.
pub(crate) fn wrap(words: &Segments, max: f32, measure: &dyn Fn(&str) -> f32) -> Vec<Segments> {
    let mut lines: Vec<Segments> = Vec::new();
    let mut used = 0.0;
    for word in words {
        let w = measure(&word.text);
        match lines.last_mut() {
            Some(line) if used + WORD_GAP + w <= max => {
                line.push(word.clone());
                used += WORD_GAP + w;
            }
            _ => {
                lines.push(vec![word.clone()]);
                used = w;
            }
        }
    }
    lines
}

/// Builds the GUI calls of one frame, at one alpha.
pub(crate) struct Pen<'a> {
    pub(crate) draws: Vec<GuiDraw>,
    pub(crate) alpha: f32,
    pub(crate) measure: &'a dyn Fn(&str) -> f32,
}

impl Pen<'_> {
    pub(crate) fn width(&self, text: &str) -> f32 {
        (self.measure)(text)
    }

    /// The tint at this alpha, or None when it's too faint to draw (the game skips text
    /// under alpha 4 too).
    pub(crate) fn visible(&self) -> Option<u32> {
        let color = opaque(0xFFFFFF, self.alpha);
        (color >> 24 >= 8).then_some(color)
    }

    pub(crate) fn sprite(&mut self, name: &'static str, x: f32, y: f32, w: i32, h: i32) {
        self.sprite_region(name, [x, y], [w, h], None);
    }

    pub(crate) fn sprite_region(
        &mut self,
        name: &'static str,
        at: [f32; 2],
        size: [i32; 2],
        region: Option<[i32; 4]>,
    ) {
        let Some(color) = self.visible() else {
            return;
        };
        self.draws.push(GuiDraw::Sprite {
            sprite: Cow::Borrowed(name),
            x: at[0].round() as i32,
            y: at[1].round() as i32,
            w: size[0],
            h: size[1],
            region,
            color,
            blend: Blend::Textured,
        });
    }

    /// `slots` slots of the hotbar sprite (182 × 22: a pixel of edge, then 20 a slot).
    pub(crate) fn strip(&mut self, x: f32, y: f32, slots: i32) {
        let body = 1 + 20 * slots;
        self.sprite_region("hud/hotbar", [x, y], [body, 22], Some([182, 22, 0, 0]));
        self.sprite_region(
            "hud/hotbar",
            [x + body as f32, y],
            [1, 22],
            Some([182, 22, 181, 0]),
        );
    }

    pub(crate) fn text(&mut self, x: f32, y: f32, span: &Span) {
        let color = opaque(span.color, self.alpha);
        if color >> 24 < 8 || span.text.is_empty() {
            return;
        }
        let text = Text::literal(span.text.as_str());
        self.draws.push(GuiDraw::Text {
            text: if span.italic { text.italic() } else { text },
            x: x.round() as i32,
            y: y.round() as i32,
            color,
            shadow: true,
        });
    }

    pub(crate) fn segments(&mut self, x: f32, y: f32, segments: &Segments) {
        let mut x = x;
        for span in segments {
            self.text(x, y, span);
            x += self.width(&span.text);
        }
    }

    /// The skin's face and hat layer, as the player list draws them.
    pub(crate) fn face(&mut self, skin: &str, x: f32, y: f32) {
        let Some(color) = self.visible() else {
            return;
        };
        for u in [8.0, 40.0] {
            self.draws.push(GuiDraw::Blit {
                texture: Cow::Owned(skin.to_owned()),
                blend: Blend::Textured,
                x: x.round() as i32,
                y: y.round() as i32,
                w: 16,
                h: 16,
                uv: [u, 8.0],
                src: [8, 8],
                tex: [64, 64],
                color,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    fn style() -> Style {
        Style {
            placement: Placement::Right,
            offset: [0.0, 0.0],
            scale: 1.0,
            opacity: 1.0,
        }
    }

    fn view() -> View {
        View {
            id: 7,
            name: "Alex".into(),
            skin: Some("skins/alex".into()),
            game_mode: Some("Survival".into()),
            distance: 8.0,
            ping: Some(42),
            health: Some(Health {
                current: 20.0,
                max: Some(20.0),
                absorption: 0.0,
            }),
            ..View::default()
        }
    }

    struct Clock(f64);

    impl Clock {
        fn frame(&mut self, view: &View, anim: &mut Anim) -> Painted {
            self.0 += 1.0 / 60.0;
            anim.show(true, 1.0 / 60.0);
            paint(
                view,
                anim,
                &style(),
                Lang::En,
                [480.0, 270.0],
                self.0,
                1.0 / 60.0,
                &measure,
            )
        }
        fn run(&mut self, view: &View, anim: &mut Anim, seconds: f64) -> Painted {
            let mut last = self.frame(view, anim);
            for _ in 1..(seconds * 60.0) as usize {
                last = self.frame(view, anim);
            }
            last
        }
    }

    fn texts(p: &Painted) -> Vec<String> {
        p.draws
            .iter()
            .filter_map(|d| match d {
                GuiDraw::Text { text, .. } => Some(text.plain()),
                _ => None,
            })
            .collect()
    }

    fn sprites(p: &Painted) -> Vec<&str> {
        p.draws
            .iter()
            .filter_map(|d| match d {
                GuiDraw::Sprite { sprite, .. } => Some(sprite.as_ref()),
                _ => None,
            })
            .collect()
    }

    fn origin(p: &Painted) -> [f32; 2] {
        let GuiDraw::Push(ops) = &p.draws[0] else {
            panic!("the card starts with its place")
        };
        let PoseOp::Translate(x, y) = ops[0] else {
            panic!("a translation first")
        };
        [x, y]
    }

    #[test]
    fn the_card_is_a_tooltip_box_that_slides_in_and_settles() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let first = clock.frame(&view(), &mut anim);
        let settled = clock.run(&view(), &mut anim, 2.0);
        assert!(origin(&first)[0] > origin(&settled)[0]);
        assert_eq!(anim.presence, 1.0);
        let names: Vec<&str> = settled.boxes.iter().map(|b| b.0).collect();
        assert_eq!(
            names,
            [
                "gui/sprites/tooltip/background",
                "gui/sprites/tooltip/frame"
            ]
        );
        // The box's sprites reach 12 pixels around the content, their margin included.
        let [x, y] = origin(&settled);
        assert_eq!(&settled.boxes[1].1[..2], &[x - 12.0, y - 12.0]);
        let texts = texts(&settled);
        for expected in ["Alex", "42 ms", "Survival"] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected} in {texts:?}"
            );
        }
        assert!(sprites(&settled).contains(&"icon/ping_5"));
        assert_eq!(settled.draws.last(), Some(&GuiDraw::Pop));
    }

    #[test]
    fn hearts_are_the_health_bars() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        v.health = Some(Health {
            current: 15.0,
            max: Some(20.0),
            absorption: 4.0,
        });
        let p = clock.run(&v, &mut anim, 1.0);
        let s = sprites(&p);
        let count = |name: &str| s.iter().filter(|n| **n == name).count();
        // 10 containers and 2 for the absorption; 7 full hearts and a half, 2 golden.
        assert_eq!(count("hud/heart/container"), 12);
        assert_eq!(count("hud/heart/full"), 7);
        assert_eq!(count("hud/heart/half"), 1);
        assert_eq!(count("hud/heart/absorbing_full"), 2);
    }

    #[test]
    fn a_hit_blinks_the_hearts_with_the_old_health_under_them_for_a_second() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        clock.run(&v, &mut anim, 1.0);
        v.health = Some(Health {
            current: 12.0,
            max: Some(20.0),
            absorption: 0.0,
        });
        let (mut blinked, mut old_health_shown) = (false, false);
        for _ in 0..60 {
            let p = clock.frame(&v, &mut anim);
            let s = sprites(&p);
            blinked |= s.contains(&"hud/heart/container_blinking");
            // The lost hearts, blinking white: all 10 behind the 6 left.
            old_health_shown |= s
                .iter()
                .filter(|n| **n == "hud/heart/full_blinking")
                .count()
                == 10;
        }
        assert!(blinked && old_health_shown);
        // After its second, the old health is gone and the blink with it.
        let p = clock.run(&v, &mut anim, 1.5);
        let s = sprites(&p);
        assert!(!s.iter().any(|n| n.contains("blinking")));
        assert_eq!(s.iter().filter(|n| **n == "hud/heart/full").count(), 6);
    }

    #[test]
    fn hardcore_and_frozen_hearts() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        v.hearts = HeartStyle {
            hardcore: true,
            frozen: true,
        };
        let p = clock.run(&v, &mut anim, 1.0);
        let s = sprites(&p);
        assert!(s.contains(&"hud/heart/container_hardcore"));
        assert!(s.contains(&"hud/heart/frozen_hardcore_full"));
    }

    #[test]
    fn items_sit_in_the_hotbars_slots_the_main_hand_selected() {
        let stack = |item: &str| ItemStack {
            item: item.into(),
            count: 1,
            damage: 0,
            extra: None,
        };
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        v.items = vec![
            (Slot::Main, stack("minecraft:diamond_sword")),
            (Slot::Off, stack("minecraft:shield")),
            (Slot::Armor(0), stack("minecraft:iron_helmet")),
            (Slot::Armor(3), stack("minecraft:iron_boots")),
        ];
        let p = clock.run(&v, &mut anim, 1.0);
        let s = sprites(&p);
        assert!(s.contains(&"hud/hotbar_offhand_left"));
        assert!(s.contains(&"hud/hotbar_selection"));
        assert_eq!(s.iter().filter(|n| **n == "hud/hotbar").count(), 4);
        let [x, _] = origin(&p);
        let xs: Vec<f32> = p.items.iter().map(|i| i.1[0] - x).collect();
        // Main hand in the selected slot, off hand in its own, armor head and feet.
        assert_eq!(xs, [32.0, 3.0, 60.0, 120.0]);
        assert!(p.items.iter().all(|i| i.2 == 16.0));
        v.swing = 1.0;
        assert!(clock.frame(&v, &mut anim).items[0].2 > 19.0);
    }

    #[test]
    fn rows_open_and_close_in_place() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        v.mount = Some(vec![Span::new("Horse", WHITE)]);
        v.position = Some([1.0, 64.0, 2.0]);
        clock.run(&v, &mut anim, 1.0);
        let keys = |a: &Anim| a.rows.iter().map(|r| r.key).collect::<Vec<_>>();
        assert_eq!(
            keys(&anim),
            [
                RowKey::Header,
                RowKey::Hearts,
                RowKey::Mount,
                RowKey::Coordinates
            ]
        );
        v.mount = None;
        clock.frame(&v, &mut anim);
        assert_eq!(keys(&anim)[2], RowKey::Mount);
        assert!(anim.rows[2].open < 1.0);
        clock.run(&v, &mut anim, 1.0);
        assert_eq!(
            keys(&anim),
            [RowKey::Header, RowKey::Hearts, RowKey::Coordinates]
        );
    }

    #[test]
    fn renamed_items_are_italic() {
        let (mut clock, mut anim) = (Clock(10.0), Anim::default());
        let mut v = view();
        v.main_details = vec![vec![Span::new("Excalibur", AQUA).italic()]];
        let p = clock.run(&v, &mut anim, 1.0);
        assert!(p.draws.iter().any(|d| matches!(
            d,
            GuiDraw::Text { text, .. } if text.plain() == "Excalibur" && text.is_italic()
        )));
    }

    #[test]
    fn long_lines_are_cut_and_words_wrap() {
        let long: Segments = vec![Span::new("x".repeat(60), WHITE)];
        let cut = truncate(&long, 100.0, &measure);
        assert!(measure(&cut[0].text) <= 100.0);
        assert!(cut[0].text.ends_with('…'));
        let words: Segments = (0..6)
            .map(|i| Span::new(format!("word{i}"), WHITE))
            .collect();
        let lines = wrap(&words, 100.0, &measure);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| {
            l.iter().map(|s| measure(&s.text)).sum::<f32>() + WORD_GAP * (l.len() - 1) as f32
                <= 100.0
        }));
    }

    #[test]
    fn ping_bars_follow_the_player_list() {
        assert_eq!(ping_sprite(-1), "icon/ping_unknown");
        assert_eq!(ping_sprite(149), "icon/ping_5");
        assert_eq!(ping_sprite(1000), "icon/ping_1");
    }
}
