//! The demo's HUD (PLAN.md P1.12) and its quick-select ring on screen (P1.9): a hero's
//! nanotech and bolts; the gun in his hand, or dimmed the one the fire button brings out,
//! with its name at its level, its tier, ammo and XP; warnings when its ammo runs low or
//! out, a flash when ammo goes up, a notice when a gun levels up; and the ring while it is
//! open.
//!
//! The HUD only shows the simulation, so it runs in `Update`. Each frame plain functions
//! work out a [`HudView`] from a hero's components ([`hud_view`]), and the screen follows
//! it. The layout, timings and each gun's icon come from `assets/hud.ron` ([`HudLayout`]);
//! the icons are coloured shapes with letters, the demo's own.
//!
//! Each player's HUD is a tree of its own under a [`PlayerHud`] root covering the screen
//! ([`spawn_hud`]), so a second player's can be given its share of the screen later.

use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_4, TAU};

use bevy::{math::ops, prelude::*};
use rac_combat::{
    Armory, Arsenal, GunEvent, GunEventKind, WeaponId,
    weapons::{
        FireMode, Hand, WeaponDef,
        ring::{RINGS, SLOTS},
    },
};
use rac_hero::{Health, Hero};
use serde::{Deserialize, Serialize};

use crate::{Bolts, DemoEntity};

/// The bundled layout.
pub const HUD_RON: &str = include_str!("../assets/hud.ron");

/// The levels' names, V1 to Omega.
const TIERS: [&str; 8] = ["V1", "V2", "V3", "V4", "V5", "Mega", "Giga", "Omega"];

/// A notice fades out over its last this many seconds.
const NOTICE_FADE: f32 = 0.5;

/// An sRGB colour, 0 to 1.
pub type Rgb = [f32; 3];
/// An sRGB colour with its alpha.
pub type Rgba = [f32; 4];

/// Where the HUD's parts go, how long its notices last and how each gun is drawn.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudLayout {
    /// From the screen's edges.
    pub margin: f32,
    pub text: Rgb,
    pub text_size: f32,
    pub small_size: f32,
    /// Behind each group of parts.
    pub panel: Rgba,
    /// How bright the gun the fire button brings out is drawn while the wrench is out.
    pub dimmed: f32,
    pub health: HealthLayout,
    pub bolts: BoltsLayout,
    pub gun: GunLayout,
    /// At or below this share of its most, a gun's ammo is low.
    pub low_ammo: f32,
    pub low_colour: Rgb,
    pub empty_colour: Rgb,
    /// Blinks a second of the ammo warning.
    pub warning_blink: f32,
    /// Ammo going up lights the count for this long.
    pub ammo_flash_seconds: f32,
    pub ammo_flash_colour: Rgb,
    /// A gun's level up stays on screen this long.
    pub level_up_seconds: f32,
    pub level_up_colour: Rgb,
    pub ring: RingLayout,
    /// The icon's shape for each fire mode.
    pub shapes: Shapes,
    /// Each gun's colour and letters, by its V1 name.
    pub icons: BTreeMap<String, Icon>,
}

/// UYA's nanotech, as pips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthLayout {
    /// A pip's width and height.
    pub pip: (f32, f32),
    pub gap: f32,
    /// A pip a point up to this many; past it, each pip stands for a share of the most.
    pub most_pips: u32,
    pub full: Rgb,
    pub lost: Rgb,
    /// At or below this much health the pips turn `low_colour`.
    pub low: u32,
    pub low_colour: Rgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoltsLayout {
    pub icon: f32,
    pub colour: Rgb,
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GunLayout {
    pub icon: f32,
    pub name_size: f32,
    pub ammo_size: f32,
    /// The XP bar's width and height.
    pub xp: (f32, f32),
    pub xp_colour: Rgb,
    pub xp_back: Rgb,
}

/// The quick-select ring round the middle of the screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RingLayout {
    /// From the middle to each slot's.
    pub radius: f32,
    pub slot: f32,
    pub icon: f32,
    pub back: Rgba,
    pub slot_back: Rgba,
    pub edge: Rgb,
    pub highlight: Rgb,
    /// The highlighted slot is drawn this much bigger.
    pub highlight_scale: f32,
}

/// A placeholder icon's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Circle,
    Square,
    Diamond,
    Bar,
    /// A circle's outline.
    Ring,
}

/// An icon's shape for each fire mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shapes {
    pub hitscan: Shape,
    pub projectile: Shape,
    pub lobbed: Shape,
    pub stream: Shape,
    pub homing: Shape,
}

/// How one gun is drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub colour: Rgb,
    /// A few letters over the shape.
    pub mark: String,
    /// Its own shape, rather than its fire mode's.
    #[serde(default)]
    pub shape: Option<Shape>,
}

#[derive(Debug, thiserror::Error)]
#[error("HUD layout: {0}")]
pub struct HudLayoutError(#[from] ron::error::SpannedError);

impl HudLayout {
    pub fn from_ron(text: &str) -> Result<Self, HudLayoutError> {
        Ok(ron::from_str(text)?)
    }

    /// How `def` is drawn: its own colour and letters, in its fire mode's shape. A gun
    /// missing from the layout is grey, with its name's initials.
    pub fn icon(&self, def: &WeaponDef) -> (Shape, Rgb, String) {
        let by_mode = match def.fire {
            FireMode::Hitscan(_) => self.shapes.hitscan,
            FireMode::Projectile(_) => self.shapes.projectile,
            FireMode::Lobbed(_) => self.shapes.lobbed,
            FireMode::Stream(_) => self.shapes.stream,
            FireMode::Homing(_) => self.shapes.homing,
        };
        match self.icons.get(&def.name) {
            Some(icon) => (
                icon.shape.unwrap_or(by_mode),
                icon.colour,
                icon.mark.clone(),
            ),
            None => (by_mode, [0.7, 0.7, 0.7], initials(&def.name)),
        }
    }
}

impl Default for HudLayout {
    fn default() -> Self {
        Self::from_ron(HUD_RON).expect("the bundled HUD layout parses")
    }
}

/// The first letters of a name's words, at most three.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(3)
        .collect()
}

/// A gun's ammo: what it has and its most at its level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ammo {
    pub now: u32,
    pub most: u32,
}

/// What the ammo count warns of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmoWarning {
    None,
    /// At or below the layout's `low_ammo` share of its most.
    Low,
    Empty,
}

impl Ammo {
    pub fn warning(self, low_share: f32) -> AmmoWarning {
        if self.now == 0 {
            AmmoWarning::Empty
        } else if self.now as f32 <= self.most as f32 * low_share {
            AmmoWarning::Low
        } else {
            AmmoWarning::None
        }
    }
}

/// One gun as the HUD shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct GunView {
    pub weapon: WeaponId,
    /// Its name at its level (`WeaponDef::name_at`).
    pub name: String,
    pub tier: &'static str,
    /// In his hand or coming out; otherwise the wrench is out and this is the gun the fire
    /// button brings out, drawn dimmed.
    pub in_hand: bool,
    /// `None` for a gun without ammo.
    pub ammo: Option<Ammo>,
    /// How far it is to the next level, 0 to 1; `None` at the end of its chain.
    pub xp: Option<f32>,
}

/// The quick-select ring on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct RingView {
    /// Which ring shows, of `rings`.
    pub ring: usize,
    pub rings: usize,
    /// Slot 0 at the top, the rest clockwise.
    pub slots: [Option<GunView>; SLOTS],
    pub highlighted: Option<usize>,
}

/// Everything one player's HUD shows.
#[derive(Debug, Clone, PartialEq)]
pub struct HudView {
    /// Health now and its most.
    pub health: Option<(u32, u32)>,
    pub bolts: Option<u32>,
    pub gun: Option<GunView>,
    pub ring: Option<RingView>,
}

/// A level's name: V1 to V5, Mega, Giga and Omega.
pub fn tier(level: u8) -> &'static str {
    TIERS.get(usize::from(level)).copied().unwrap_or("")
}

/// How far a gun at `level` with `xp` is through its level: from the XP it reached the
/// level with to the level's threshold. `None` at the end of a chain: V5 before Mega is
/// bought, and Omega.
pub fn xp_progress(def: &WeaponDef, level: u8, xp: u32, bought_level: u8) -> Option<f32> {
    let next = def.level(level).xp;
    if next == 0 || usize::from(level) + 1 >= def.levels.len() {
        return None;
    }
    // XP counts again from 0 at the bought level, Mega.
    let from = if level == 0 || level == bought_level {
        0
    } else {
        def.level(level - 1).xp
    };
    let span = next.saturating_sub(from).max(1);
    Some((xp.saturating_sub(from) as f32 / span as f32).clamp(0.0, 1.0))
}

/// `weapon` as the HUD shows it, if he owns it.
pub fn gun_view(
    arsenal: &Arsenal,
    armory: &Armory,
    weapon: WeaponId,
    in_hand: bool,
) -> Option<GunView> {
    let def = armory.get(weapon)?;
    let gun = arsenal.gun(weapon)?;
    let most = def.level(gun.level).ammo;
    Some(GunView {
        weapon,
        name: def.name_at(gun.level).to_owned(),
        tier: tier(gun.level),
        in_hand,
        ammo: (most > 0).then_some(Ammo {
            now: gun.ammo,
            most,
        }),
        xp: xp_progress(def, gun.level, gun.xp, armory.rules.bought_level),
    })
}

/// The gun the HUD shows: the one in his hand or coming out; with the wrench out, the one
/// the fire button brings out.
pub fn shown_gun(arsenal: &Arsenal, armory: &Armory) -> Option<GunView> {
    match arsenal.hand {
        Hand::Gun { weapon, .. } | Hand::Drawing { weapon, .. } => {
            gun_view(arsenal, armory, weapon, true)
        }
        Hand::Wrench => gun_view(arsenal, armory, arsenal.equipped?, false),
    }
}

/// The quick-select ring, while it is open.
pub fn ring_view(arsenal: &Arsenal, armory: &Armory) -> Option<RingView> {
    let open = arsenal.quick.open?;
    let ring = arsenal.quick.ring;
    Some(RingView {
        ring,
        rings: RINGS,
        slots: arsenal.quick.rings[ring]
            .map(|slot| slot.and_then(|weapon| gun_view(arsenal, armory, weapon, true))),
        highlighted: open.highlighted,
    })
}

/// What one hero's HUD shows.
pub fn hud_view(
    arsenal: Option<&Arsenal>,
    armory: &Armory,
    health: Option<&Health>,
    bolts: Option<&Bolts>,
) -> HudView {
    HudView {
        health: health.map(|health| (health.current, health.max)),
        bolts: bolts.map(|bolts| bolts.0),
        gun: arsenal.and_then(|arsenal| shown_gun(arsenal, armory)),
        ring: arsenal.and_then(|arsenal| ring_view(arsenal, armory)),
    }
}

/// The health pips to draw and how many are lit: a pip a point up to `most_pips`; past
/// that, `most_pips` pips, each lit while any of its share is left.
pub fn health_pips(current: u32, max: u32, most_pips: u32) -> (u32, u32) {
    let pips = max.min(most_pips);
    if pips == 0 {
        return (0, 0);
    }
    let lit = (u64::from(current.min(max)) * u64::from(pips)).div_ceil(u64::from(max));
    (pips, lit as u32)
}

/// Whether the ammo shown went up since it was last seen: the same gun, with more.
pub fn ammo_rose(before: Option<(WeaponId, u32)>, now: Option<(WeaponId, u32)>) -> bool {
    matches!((before, now), (Some((was, before)), Some((is, now))) if was == is && now > before)
}

/// The notice for a gun reaching `level`.
pub fn level_up_notice(def: &WeaponDef, level: u8) -> String {
    format!("{} upgraded to {}", def.name_at(level), tier(level))
}

/// A count with its thousands apart: 12,345.
pub fn grouped(count: u32) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Draws each player's HUD for their hero.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudLayout>()
            .add_systems(Update, (note_level_ups, draw_huds).chain());
    }
}

/// The root of one player's HUD, and what it remembers between frames.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct PlayerHud {
    pub player: usize,
    /// The gun and ammo last shown, to see the ammo go up.
    seen_ammo: Option<(WeaponId, u32)>,
    /// Seconds left of the ammo count's flash.
    flash: f32,
    /// A level up on screen, and its seconds left.
    notice: Option<(String, f32)>,
}

impl PlayerHud {
    pub fn new(player: usize) -> Self {
        Self {
            player,
            seen_ammo: None,
            flash: 0.0,
            notice: None,
        }
    }
}

/// One part of a player's HUD that changes.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct HudPart {
    player: usize,
    part: Part,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Health,
    Pip(u32),
    HealthCount,
    Bolts,
    BoltsCount,
    Gun,
    GunShape,
    GunMark,
    GunName,
    Ammo,
    XpBar,
    XpFill,
    Warning,
    Notice,
    Ring,
    RingLabel,
    Slot(usize),
    SlotShape(usize),
    SlotMark(usize),
}

fn rgb([r, g, b]: Rgb) -> Color {
    Color::srgb(r, g, b)
}

fn rgba([r, g, b, a]: Rgba) -> Color {
    Color::srgba(r, g, b, a)
}

/// Text that stays readable over the bright sky.
fn label(text: &str, size: f32, colour: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(colour),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::srgba(0.0, 0.0, 0.0, 0.8),
        },
    )
}

/// A box behind a group of parts.
fn panel(layout: &HudLayout) -> (Node, BackgroundColor) {
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: UiRect::axes(px(12), px(8)),
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BackgroundColor(rgba(layout.panel)),
    )
}

/// An icon's box of `size`: its shape, and its letters over it.
fn icon_box(parent: &mut ChildSpawnerCommands, size: f32, shape: HudPart, mark: HudPart) {
    parent
        .spawn(Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|icon| {
            icon.spawn((shape, Node::default()));
            icon.spawn((
                mark,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(size * 0.3),
                    ..default()
                },
            ));
        });
}

/// Spawns `player`'s HUD, covering the screen: health at the top left, bolts and the gun
/// at the top right (clear of the app's controls at the bottom left), and the ring round
/// the middle while it is open.
pub fn spawn_hud(commands: &mut Commands, player: usize, layout: &HudLayout) -> Entity {
    let part = |part| HudPart { player, part };
    let text = rgb(layout.text);
    commands
        .spawn((
            Name::new(format!("player {player} hud")),
            DemoEntity,
            PlayerHud::new(player),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            let health = &layout.health;
            let (mut node, back) = panel(layout);
            node.position_type = PositionType::Absolute;
            node.left = px(layout.margin);
            node.top = px(layout.margin);
            root.spawn((part(Part::Health), node, back))
                .with_children(|panel| {
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: px(health.gap),
                            ..default()
                        })
                        .with_children(|pips| {
                            for pip in 0..health.most_pips {
                                pips.spawn((
                                    part(Part::Pip(pip)),
                                    Node {
                                        width: px(health.pip.0),
                                        height: px(health.pip.1),
                                        border_radius: BorderRadius::all(px(3)),
                                        ..default()
                                    },
                                    BackgroundColor(rgb(health.full)),
                                ));
                            }
                        });
                    panel.spawn((part(Part::HealthCount), label("", layout.small_size, text)));
                });

            root.spawn(Node {
                position_type: PositionType::Absolute,
                right: px(layout.margin),
                top: px(layout.margin),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(8),
                ..default()
            })
            .with_children(|column| {
                column
                    .spawn((part(Part::Bolts), panel(layout)))
                    .with_children(|bolts| {
                        // A bolt: a gold square turned on its corner.
                        let size = layout.bolts.icon;
                        bolts.spawn((
                            Node {
                                width: px(size),
                                height: px(size),
                                border_radius: BorderRadius::all(px(size * 0.15)),
                                ..default()
                            },
                            BackgroundColor(rgb(layout.bolts.colour)),
                            UiTransform::from_rotation(Rot2::radians(FRAC_PI_4)),
                        ));
                        bolts.spawn((
                            part(Part::BoltsCount),
                            label("0", layout.bolts.size, rgb(layout.bolts.colour)),
                        ));
                    });

                let gun = &layout.gun;
                let (mut node, back) = panel(layout);
                node.column_gap = px(12);
                column
                    .spawn((part(Part::Gun), node, back))
                    .with_children(|panel| {
                        icon_box(panel, gun.icon, part(Part::GunShape), part(Part::GunMark));
                        panel
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::FlexEnd,
                                row_gap: px(3),
                                ..default()
                            })
                            .with_children(|details| {
                                details
                                    .spawn((part(Part::GunName), label("", gun.name_size, text)));
                                details.spawn((part(Part::Ammo), label("", gun.ammo_size, text)));
                                details
                                    .spawn((
                                        part(Part::XpBar),
                                        Node {
                                            width: px(gun.xp.0),
                                            height: px(gun.xp.1),
                                            border_radius: BorderRadius::all(px(gun.xp.1 / 2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(rgb(gun.xp_back)),
                                    ))
                                    .with_children(|bar| {
                                        bar.spawn((
                                            part(Part::XpFill),
                                            Node {
                                                width: percent(0),
                                                height: percent(100),
                                                border_radius: BorderRadius::all(
                                                    px(gun.xp.1 / 2.0),
                                                ),
                                                ..default()
                                            },
                                            BackgroundColor(rgb(gun.xp_colour)),
                                        ));
                                    });
                            });
                    });
                column.spawn((part(Part::Warning), label("", layout.text_size, text)));
                column.spawn((part(Part::Notice), label("", layout.text_size, text)));
            });

            let ring = &layout.ring;
            root.spawn((
                part(Part::Ring),
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    top: percent(50),
                    width: px(0),
                    height: px(0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|centre| {
                let reach = ring.radius + ring.slot * 0.8;
                centre.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-reach),
                        top: px(-reach),
                        width: px(2.0 * reach),
                        height: px(2.0 * reach),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(rgba(ring.back)),
                ));
                for slot in 0..SLOTS {
                    // Slot 0 at the top, then clockwise, as the stick points.
                    let angle = TAU * slot as f32 / SLOTS as f32;
                    let x = ring.radius * ops::sin(angle);
                    let y = -ring.radius * ops::cos(angle);
                    centre
                        .spawn((
                            part(Part::Slot(slot)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(x - ring.slot / 2.0),
                                top: px(y - ring.slot / 2.0),
                                width: px(ring.slot),
                                height: px(ring.slot),
                                border: UiRect::all(px(3)),
                                border_radius: BorderRadius::all(px(12)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(rgba(ring.slot_back)),
                            BorderColor::all(rgb(ring.edge)),
                        ))
                        .with_children(|slot_box| {
                            icon_box(
                                slot_box,
                                ring.icon,
                                part(Part::SlotShape(slot)),
                                part(Part::SlotMark(slot)),
                            );
                        });
                }
                let width = 2.0 * (ring.radius - ring.slot * 0.6);
                centre.spawn((
                    part(Part::RingLabel),
                    label("", layout.small_size, text),
                    TextLayout::justify(Justify::Center),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-width / 2.0),
                        top: px(-1.6 * layout.small_size),
                        width: px(width),
                        ..default()
                    },
                ));
            });
        })
        .id()
}

/// Puts each gun's level up on its player's HUD.
fn note_level_ups(
    layout: Res<HudLayout>,
    armory: Res<Armory>,
    mut events: MessageReader<GunEvent>,
    mut huds: Query<&mut PlayerHud>,
) {
    for event in events.read() {
        let GunEventKind::LevelUp { level } = event.kind else {
            continue;
        };
        let Some(def) = armory.get(event.weapon) else {
            continue;
        };
        for mut hud in huds.iter_mut().filter(|hud| hud.player == event.player) {
            hud.notice = Some((level_up_notice(def, level), layout.level_up_seconds));
        }
    }
}

/// One player's view this frame, with the HUD's own timers.
struct Shown {
    player: usize,
    view: HudView,
    /// The ammo flash's share left, 1 to 0.
    flash: f32,
    notice: Option<(String, f32)>,
}

type PartItem<'a> = (
    &'a HudPart,
    &'a mut Node,
    &'a mut Visibility,
    &'a mut BackgroundColor,
    &'a mut BorderColor,
    &'a mut UiTransform,
    Option<&'a mut Text>,
    Option<&'a mut TextColor>,
);

/// Works out each player's view and makes the screen follow it.
fn draw_huds(
    time: Res<Time>,
    layout: Res<HudLayout>,
    armory: Res<Armory>,
    heroes: Query<(&Hero, Option<&Arsenal>, Option<&Health>, Option<&Bolts>)>,
    mut huds: Query<&mut PlayerHud>,
    mut parts: Query<PartItem>,
) {
    let seconds = time.delta_secs();
    let mut shown = Vec::new();
    for mut hud in &mut huds {
        let Some((_, arsenal, health, bolts)) =
            heroes.iter().find(|(hero, ..)| hero.player == hud.player)
        else {
            continue;
        };
        let view = hud_view(arsenal, &armory, health, bolts);
        let ammo = view
            .gun
            .as_ref()
            .and_then(|gun| Some((gun.weapon, gun.ammo?.now)));
        hud.flash = if ammo_rose(hud.seen_ammo, ammo) {
            layout.ammo_flash_seconds
        } else {
            (hud.flash - seconds).max(0.0)
        };
        hud.seen_ammo = ammo;
        let over = hud.notice.as_mut().is_some_and(|(_, left)| {
            *left -= seconds;
            *left <= 0.0
        });
        if over {
            hud.notice = None;
        }
        shown.push(Shown {
            player: hud.player,
            view,
            flash: hud.flash / layout.ammo_flash_seconds.max(1e-3),
            notice: hud.notice.clone(),
        });
    }
    let blink_on = (time.elapsed_secs() * layout.warning_blink).fract() < 0.65;

    for (part, mut node, mut visibility, mut background, mut border, mut transform, text, colour) in
        &mut parts
    {
        let Some(shown) = shown.iter().find(|shown| shown.player == part.player) else {
            continue;
        };
        let mut look = Look {
            visibility: &mut visibility,
            text,
            colour,
        };
        let view = &shown.view;
        match part.part {
            Part::Health => look.show(view.health.is_some()),
            Part::Pip(pip) => {
                let (current, max) = view.health.unwrap_or((0, 0));
                let health = &layout.health;
                let (pips, lit) = health_pips(current, max, health.most_pips);
                look.show(pip < pips);
                let colour = if pip >= lit {
                    health.lost
                } else if current <= health.low {
                    health.low_colour
                } else {
                    health.full
                };
                background.set_if_neq(BackgroundColor(rgb(colour)));
            }
            Part::HealthCount => {
                let (current, max) = view.health.unwrap_or((0, 0));
                look.text(&format!("{current}/{max}"));
            }
            Part::Bolts => look.show(view.bolts.is_some()),
            Part::BoltsCount => look.text(&grouped(view.bolts.unwrap_or(0))),
            Part::Gun => look.show(view.gun.is_some()),
            Part::GunShape | Part::GunMark => {
                let Some(gun) = &view.gun else {
                    continue;
                };
                let alpha = if gun.in_hand { 1.0 } else { layout.dimmed };
                let icon = IconLook::of(&layout, &armory, gun.weapon);
                if part.part == Part::GunShape {
                    icon.paint(
                        layout.gun.icon,
                        alpha,
                        &mut node,
                        &mut background,
                        &mut border,
                        &mut transform,
                    );
                } else {
                    look.text(&icon.mark);
                    look.colour(icon.mark_colour().with_alpha(alpha));
                }
            }
            Part::GunName => {
                let Some(gun) = &view.gun else {
                    continue;
                };
                look.text(&format!("{} {}", gun.name, gun.tier));
                let alpha = if gun.in_hand { 1.0 } else { layout.dimmed };
                look.colour(rgb(layout.text).with_alpha(alpha));
            }
            Part::Ammo => {
                let Some((gun, ammo)) = view.gun.as_ref().and_then(|gun| Some((gun, gun.ammo?)))
                else {
                    look.show(false);
                    continue;
                };
                look.show(true);
                look.text(&format!("{}/{}", ammo.now, ammo.most));
                let warned = match ammo.warning(layout.low_ammo) {
                    AmmoWarning::None => layout.text,
                    AmmoWarning::Low => layout.low_colour,
                    AmmoWarning::Empty => layout.empty_colour,
                };
                let colour = rgb(warned).mix(&rgb(layout.ammo_flash_colour), shown.flash);
                let alpha = if gun.in_hand { 1.0 } else { layout.dimmed };
                look.colour(colour.with_alpha(alpha));
            }
            Part::XpBar => look.show(view.gun.as_ref().is_some_and(|gun| gun.xp.is_some())),
            Part::XpFill => {
                let share = view.gun.as_ref().and_then(|gun| gun.xp).unwrap_or(0.0);
                let width = percent(share * 100.0);
                if node.width != width {
                    node.width = width;
                }
            }
            Part::Warning => {
                let warning = view
                    .gun
                    .as_ref()
                    .and_then(|gun| gun.ammo)
                    .map_or(AmmoWarning::None, |ammo| ammo.warning(layout.low_ammo));
                let (words, colour) = match warning {
                    AmmoWarning::None => ("", layout.text),
                    AmmoWarning::Low => ("Low ammo", layout.low_colour),
                    AmmoWarning::Empty => ("Out of ammo", layout.empty_colour),
                };
                look.show(warning != AmmoWarning::None && blink_on);
                look.text(words);
                look.colour(rgb(colour));
            }
            Part::Notice => match &shown.notice {
                Some((words, left)) => {
                    look.show(true);
                    look.text(words);
                    let fade = (left / NOTICE_FADE).clamp(0.0, 1.0);
                    look.colour(rgb(layout.level_up_colour).with_alpha(fade));
                }
                None => look.show(false),
            },
            Part::Ring => look.show(view.ring.is_some()),
            Part::RingLabel => {
                let Some(ring) = &view.ring else {
                    continue;
                };
                let chosen = ring
                    .highlighted
                    .and_then(|slot| ring.slots[slot].as_ref())
                    .map_or(String::new(), |gun| {
                        let ammo = gun.ammo.map_or(String::new(), |ammo| {
                            format!("\n{}/{}", ammo.now, ammo.most)
                        });
                        format!("\n{} {}{ammo}", gun.name, gun.tier)
                    });
                look.text(&format!("Ring {} of {}{chosen}", ring.ring + 1, ring.rings));
            }
            Part::Slot(slot) => {
                let Some(ring) = &view.ring else {
                    continue;
                };
                let lit = ring.highlighted == Some(slot);
                let edge = if lit {
                    layout.ring.highlight
                } else {
                    layout.ring.edge
                };
                border.set_if_neq(BorderColor::all(rgb(edge)));
                let scale = if lit {
                    layout.ring.highlight_scale
                } else {
                    1.0
                };
                transform.set_if_neq(UiTransform::from_scale(Vec2::splat(scale)));
            }
            Part::SlotShape(slot) | Part::SlotMark(slot) => {
                let gun = view
                    .ring
                    .as_ref()
                    .and_then(|ring| ring.slots[slot].as_ref());
                let Some(gun) = gun else {
                    look.show(false);
                    continue;
                };
                look.show(true);
                // A gun out of ammo can still be chosen, but shows it.
                let alpha = if gun.ammo.is_some_and(|ammo| ammo.now == 0) {
                    layout.dimmed
                } else {
                    1.0
                };
                let icon = IconLook::of(&layout, &armory, gun.weapon);
                if matches!(part.part, Part::SlotShape(_)) {
                    icon.paint(
                        layout.ring.icon,
                        alpha,
                        &mut node,
                        &mut background,
                        &mut border,
                        &mut transform,
                    );
                } else {
                    look.text(&icon.mark);
                    look.colour(icon.mark_colour().with_alpha(alpha));
                }
            }
        }
    }
}

/// A part's visibility and text, changed only when they differ.
struct Look<'a, 'w> {
    visibility: &'a mut Mut<'w, Visibility>,
    text: Option<Mut<'w, Text>>,
    colour: Option<Mut<'w, TextColor>>,
}

impl Look<'_, '_> {
    fn show(&mut self, on: bool) {
        self.visibility.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }

    fn text(&mut self, words: &str) {
        if let Some(text) = &mut self.text
            && text.0 != words
        {
            text.0 = words.to_owned();
        }
    }

    fn colour(&mut self, colour: Color) {
        if let Some(text_colour) = &mut self.colour {
            text_colour.set_if_neq(TextColor(colour));
        }
    }
}

/// A gun's icon as drawn.
struct IconLook {
    shape: Shape,
    colour: Color,
    mark: String,
}

impl IconLook {
    fn of(layout: &HudLayout, armory: &Armory, weapon: WeaponId) -> Self {
        let (shape, colour, mark) = match armory.get(weapon) {
            Some(def) => layout.icon(def),
            None => (Shape::Square, [0.7, 0.7, 0.7], String::new()),
        };
        Self {
            shape,
            colour: rgb(colour),
            mark,
        }
    }

    /// Dark letters on a filled shape; an outline's letters take its colour.
    fn mark_colour(&self) -> Color {
        match self.shape {
            Shape::Ring => self.colour,
            _ => Color::srgb(0.06, 0.07, 0.1),
        }
    }

    /// Sizes and colours the shape's node, centred in an icon box of `size`.
    fn paint(
        &self,
        size: f32,
        alpha: f32,
        node: &mut Mut<Node>,
        background: &mut Mut<BackgroundColor>,
        border: &mut Mut<BorderColor>,
        transform: &mut Mut<UiTransform>,
    ) {
        let (width, height, radius, outline, turn) = match self.shape {
            Shape::Circle => (0.8, 0.8, BorderRadius::MAX, 0.0, 0.0),
            Shape::Square => (0.72, 0.72, BorderRadius::all(px(size * 0.1)), 0.0, 0.0),
            Shape::Diamond => (
                0.66,
                0.66,
                BorderRadius::all(px(size * 0.06)),
                0.0,
                FRAC_PI_4,
            ),
            Shape::Bar => (1.0, 0.42, BorderRadius::all(px(size * 0.21)), 0.0, 0.0),
            Shape::Ring => (0.84, 0.84, BorderRadius::MAX, size * 0.14, 0.0),
        };
        let (width, height) = (width * size, height * size);
        node.set_if_neq(Node {
            position_type: PositionType::Absolute,
            left: px((size - width) / 2.0),
            top: px((size - height) / 2.0),
            width: px(width),
            height: px(height),
            border: UiRect::all(px(outline)),
            border_radius: radius,
            ..default()
        });
        let colour = self.colour.with_alpha(alpha);
        let (fill, edge) = if outline > 0.0 {
            (Color::NONE, colour)
        } else {
            (colour, Color::NONE)
        };
        background.set_if_neq(BackgroundColor(fill));
        border.set_if_neq(BorderColor::all(edge));
        transform.set_if_neq(UiTransform::from_rotation(Rot2::radians(turn)));
    }
}
