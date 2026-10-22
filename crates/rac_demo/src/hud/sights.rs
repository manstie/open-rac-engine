//! The guns' aim indicators on each player's HUD (`weapons.md`, "Aim indicators"; PLAN.md
//! P1.12), drawn over the frame as UYA draws them:
//! - the lock-on reticle: three turning rings round the locked-on target, with a health arc
//!   below it;
//! - the crosshair at the view's centre, pulsing, and red over a target;
//! - the marks the guns put on their targets;
//! - the Flux Rifle's scope, with its ring blinking over an enemy.
//!
//! What to mark comes from the simulation ([`Arsenal::sights`], [`LockOn`], the free look's
//! target weight) and how from each gun's [`Sight`]. The fades, spins and pulses only show
//! it, so they run here by the frame, at the game's rates for its 60 frames a second
//! ([`SightsClock`]). Sizes are the game's pixels on its 512 × 416 frame, scaled by the
//! viewport's height.
//!
//! Plain functions work out a frame's [`Sprite`]s ([`sprites`]); `draw_sights` puts them on
//! a pool of image nodes under each [`PlayerHud`], beneath its panels. The images are the
//! demo's own placeholders, painted at start-up and keyed by the game's effect-texture
//! numbers ([`SightImages`]), so the disc's textures can take their place (P3.18).

use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::{
    asset::RenderAssetUsages,
    math::ops,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use rac_combat::{
    Armory, Arsenal, Hitpoints, Hostile, Hurtbox,
    weapons::{Crosshair, Hand, Mark, Showing, Sight, Sights},
};
use rac_hero::{CameraSetting, FollowCamera, Hero, HeroBody, HeroState, LockOn};

use super::PlayerHud;

/// The game's frame is this many pixels high.
const FRAME_HEIGHT: f32 = 416.0;
/// The game's frames a second, which its rates are given per.
const FRAME_RATE: f32 = 60.0;
/// The most marks the guns queue in a frame (0x1E5200).
const MOST_MARKS: usize = 14;
/// The health arc's quads (`hud_arc`, 0x404F68).
const ARC_DOTS: usize = 16;
/// The sprites a player's HUD keeps: the scope's three, the reticle's rings and arc, the
/// marks, the crosshair and the scope's ring.
const POOL: usize = 3 + 3 + ARC_DOTS + MOST_MARKS + 2;

/// The sights' textures, by their numbers in a level's effect table.
pub mod texture {
    /// A soft dot: the health arc's.
    pub const DOT: u16 = 0x05;
    /// A downward triangle: the Spitting Hydra's marks.
    pub const TRIANGLE: u16 = 0x1F;
    /// A segmented ring: the Flux Rifle's scope over an enemy.
    pub const SCOPE_RING: u16 = 0x20;
    /// A ring with four ticks: the shared target mark.
    pub const MARK: u16 = 0x26;
    /// The lock-on reticle's rings: turning with its spin, against it, and fixed.
    pub const LOCK_OUTER: u16 = 0x33;
    pub const LOCK_INNER: u16 = 0x34;
    pub const LOCK_FIXED: u16 = 0x35;
    /// The Flux Rifle's scope.
    pub const SCOPE: u16 = 0x46;
    /// Not the game's: a plain square, for the scope's sides past its lens.
    pub const PLAIN: u16 = u16::MAX;
    /// The crosshairs the bundled guns name, all placeholders.
    pub const CROSSHAIRS: [u16; 7] = [0x21, 0x25, 0x44, 0x56, 0x5A, 0x5B, 0x20];
}

/// The scope's surround, past its lens.
const SURROUND: [f32; 4] = [0.03, 0.03, 0.04, 0.94];
/// Its bezel, pink-orange.
const BEZEL: [f32; 4] = [1.0, 0.55, 0.45, 1.0];

/// A PS2 colour, where 0x80 is full (`weapons.md`, "Aim indicators").
fn ps2(r: u8, g: u8, b: u8, a: u8) -> Color {
    let full = |value: u8| (f32::from(value) / 128.0).min(1.0);
    Color::srgba(full(r), full(g), full(b), full(a))
}

/// A queued marker's colour, whose alpha the game caps at 0x40.
fn marker(r: u8, g: u8, b: u8, a: u8) -> Color {
    ps2(r, g, b, a.min(0x40))
}

/// One sprite to draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub texture: u16,
    /// Its centre and size on the screen, in logical pixels.
    pub centre: Vec2,
    pub size: Vec2,
    /// Turned clockwise, in degrees.
    pub degrees: f32,
    pub colour: Color,
}

/// What a player's sights keep between frames.
#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct SightsClock {
    /// The target the lock-on reticle is on, its fade (0 to 1) and its spin in degrees
    /// (0x1A6040–0x1A6048).
    reticle: Option<Entity>,
    fade: f32,
    spin: f32,
    /// The shared mark's spin, degrees.
    mark_spin: f32,
    /// Frames the crosshair has shown and its pulse's phase in degrees; both start again on
    /// a frame without it.
    crosshair_frames: f32,
    pulse: f32,
    /// Frames counted, for the scope ring's blink.
    frames: f32,
}

impl SightsClock {
    /// The lock-on reticle follows `lock`: it fades in 0.1 a frame, and on a change of
    /// target the old one fades out fully before the new one is taken. It spins 0.75° a
    /// frame, twice that while fading in.
    fn step_reticle(&mut self, lock: Option<Entity>, frames: f32) {
        // Tenths summed don't come to exactly 1 or 0.
        let settle = |fade: f32| {
            if fade < 1e-4 {
                0.0
            } else if fade > 1.0 - 1e-4 {
                1.0
            } else {
                fade
            }
        };
        let step = 0.1 * frames;
        if self.reticle != lock {
            self.fade = settle((self.fade - step).max(0.0));
            if self.fade == 0.0 {
                self.reticle = lock;
            }
        }
        let fading_in = self.reticle.is_some() && self.reticle == lock && self.fade < 1.0;
        if fading_in {
            self.fade = settle((self.fade + step).min(1.0));
        }
        let turn = if fading_in { 1.5 } else { 0.75 };
        self.spin = (self.spin + turn * frames) % 360.0;
    }
}

/// What a hero's sights show this frame.
#[derive(Debug, Clone, Copy)]
pub struct Aim<'a> {
    /// The sight of the gun in his hand, and its level; none with the wrench out or a gun
    /// still coming out.
    pub sight: Option<(&'a Sight, u8)>,
    /// The crosshair to draw when it shows: the gun's, or the wrench's.
    pub crosshair: Option<&'a Crosshair>,
    pub sights: &'a Sights,
    pub lock: Option<Entity>,
    /// He is in the look (UYA's state 29), and its camera looks where he does.
    pub looking: bool,
    pub look_ready: bool,
    /// He is seen through the third-person setting, whose camera is the follow camera
    /// outside the look.
    pub third_person: bool,
    /// His gun arm is raised against a wall.
    pub raised: bool,
    /// The free look's target weight ([`FollowCamera::target_weight`]).
    pub weight: f32,
    /// The scope's ray finds a live enemy within its reach.
    pub on_target: bool,
}

impl Aim<'_> {
    /// Aiming, as the game's sights test it: in the look, or seen through a camera other
    /// than the follow camera.
    fn aiming(&self) -> bool {
        self.looking || !self.third_person
    }

    fn scoped(&self) -> bool {
        self.sight
            .is_some_and(|(sight, _)| sight.scope_reach.is_some())
            && self.looking
            && self.look_ready
    }
}

/// The player's view.
pub struct Screen<'a> {
    /// The viewport, in logical pixels.
    pub rect: Rect,
    pub eye: Vec3,
    /// Where the view looks, of length 1.
    pub forward: Vec3,
    /// Where a point in the world is on the screen, in logical pixels; none behind the
    /// camera.
    pub project: &'a dyn Fn(Vec3) -> Option<Vec2>,
}

/// A target as the sights see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeenTarget {
    /// Where it stands.
    pub feet: Vec3,
    pub hurtbox: Option<Hurtbox>,
    pub hitpoints: Option<Hitpoints>,
}

impl SeenTarget {
    /// The point the marks sit on: raised by its hurtbox's height (0x4C74C0, the game's
    /// damage record's).
    fn raised(&self) -> Vec3 {
        self.feet + Vec3::Y * self.hurtbox.map_or(0.0, |hurtbox| hurtbox.height)
    }

    fn alive(&self) -> bool {
        self.hitpoints
            .is_none_or(|hitpoints| hitpoints.current > 0.0)
    }
}

/// The lock-on reticle's size, in game pixels, for a target `radius` u high at `distance`
/// u: 350 × atan(r / d), from 16 to 128.
pub fn reticle_size(radius: f32, distance: f32) -> f32 {
    (350.0 * ops::atan(radius / distance.max(1e-3))).clamp(16.0, 128.0)
}

/// The shared target mark's size, in game pixels, at `distance` u: 36 close, 10 at 150 u.
pub fn mark_size(distance: f32) -> f32 {
    40.0 * (0.9 - distance / 150.0 * 0.65).abs()
}

/// How far round the health arc goes for a target of `max` hit points, in radians: half
/// round at 6, nearly all round for many.
pub fn arc_span(max: f32) -> f32 {
    4.0 * ops::atan(max / 6.0)
}

/// The scope's ring is on for 15 of every 31 frames.
pub fn blink_on(frames: f32) -> bool {
    (frames as u32) % 31 < 15
}

/// A crosshair's colour after `frames` shown, at pulse phase `pulse` degrees, with the
/// view's target `weight` (`crosshair_colour`, 0x4C7588): solid red over a target past its
/// threshold; else fading in 5 counts a frame up to 0x40, pulsing between green and white,
/// or in its own colour.
pub fn crosshair_colour(crosshair: &Crosshair, frames: f32, pulse: f32, weight: f32) -> Color {
    if crosshair.red_above.is_some_and(|above| weight > above) {
        return ps2(0xFF, 0x00, 0x00, 0x60);
    }
    let alpha = (5.0 * frames).min(64.0) / 128.0;
    let colour = if crosshair.pulse {
        let white = 0.5 + 0.5 * ops::sin(pulse.to_radians());
        ps2(0x0F, 0xFF, 0x00, 0x80).mix(&Color::WHITE, white)
    } else {
        let [r, g, b] = crosshair.colour.unwrap_or([1.0; 3]);
        Color::srgb(r, g, b)
    };
    colour.with_alpha(alpha)
}

/// Whether a ray from `eye` along `forward` passes through any of `balls` (centre and
/// radius) within `reach`.
pub fn ray_hits(
    eye: Vec3,
    forward: Vec3,
    reach: f32,
    balls: impl IntoIterator<Item = (Vec3, f32)>,
) -> bool {
    balls.into_iter().any(|(centre, radius)| {
        let along = (centre - eye).dot(forward);
        (0.0..=reach).contains(&along) && (eye + forward * along).distance(centre) <= radius
    })
}

/// The sprites a hero's sights draw this frame, `frames` 60ths of a second after the last,
/// moving `clock` on. `seen` finds a target by its entity.
pub fn sprites(
    clock: &mut SightsClock,
    aim: &Aim,
    screen: &Screen,
    seen: &dyn Fn(Entity) -> Option<SeenTarget>,
    frames: f32,
) -> Vec<Sprite> {
    let mut out = Vec::new();
    let scale = screen.rect.height() / FRAME_HEIGHT;
    let centre = screen.rect.center();
    clock.frames += frames;
    clock.mark_spin = (clock.mark_spin + 2.0 * frames) % 360.0;

    // The Flux Rifle's scope, under the rest.
    let scoped = aim.scoped();
    if scoped {
        scope(&mut out, screen.rect);
    }

    if clock.reticle.is_some_and(|target| seen(target).is_none()) {
        clock.reticle = None;
        clock.fade = 0.0;
    }
    clock.step_reticle(aim.lock.filter(|&target| seen(target).is_some()), frames);
    if let Some(target) = clock.reticle.and_then(seen)
        && clock.fade > 0.0
    {
        reticle(&mut out, clock, &target, screen, scale);
    }

    let marks = marks(clock, aim, screen, seen, scale);
    out.extend(marks.into_iter().take(MOST_MARKS));

    let crosshair = aim.crosshair.filter(|crosshair| match crosshair.when {
        Showing::Aiming => aim.aiming(),
        Showing::AimingUnlocked => aim.aiming() && aim.lock.is_none(),
        Showing::Look => aim.looking,
    });
    match crosshair {
        Some(crosshair) => {
            clock.crosshair_frames += frames;
            // 150° a second.
            clock.pulse = (clock.pulse + 2.5 * frames) % 360.0;
            out.push(Sprite {
                texture: crosshair.texture,
                centre,
                size: Vec2::new(crosshair.size.0, crosshair.size.1) * scale,
                degrees: crosshair.turn_degrees,
                colour: crosshair_colour(
                    crosshair,
                    clock.crosshair_frames,
                    clock.pulse,
                    aim.weight,
                ),
            });
        }
        None => {
            clock.crosshair_frames = 0.0;
            clock.pulse = 0.0;
        }
    }

    if scoped && aim.on_target && aim.sights.ready && blink_on(clock.frames) {
        out.push(Sprite {
            texture: texture::SCOPE_RING,
            centre,
            size: Vec2::splat(32.0 * scale),
            degrees: 0.0,
            colour: ps2(0xFF, 0x00, 0x00, 0x20),
        });
    }
    out
}

/// The scope over the screen: its lens, a square as high as the screen at its centre, with
/// the dark surround beyond it.
fn scope(out: &mut Vec<Sprite>, rect: Rect) {
    let side = rect.height();
    let centre = rect.center();
    out.push(Sprite {
        texture: texture::SCOPE,
        centre,
        size: Vec2::splat(side),
        degrees: 0.0,
        colour: Color::WHITE,
    });
    let wing = (rect.width() - side) / 2.0;
    if wing > 0.0 {
        let [r, g, b, a] = SURROUND;
        for sign in [-1.0, 1.0] {
            out.push(Sprite {
                texture: texture::PLAIN,
                centre: centre + Vec2::X * sign * (side + wing) / 2.0,
                // A pixel over, so no seam shows.
                size: Vec2::new(wing + 1.0, side),
                degrees: 0.0,
                colour: Color::srgba(r, g, b, a),
            });
        }
    }
}

/// The lock-on reticle (`lock_reticle`, 0x3F4700) on `target`.
fn reticle(
    out: &mut Vec<Sprite>,
    clock: &SightsClock,
    target: &SeenTarget,
    screen: &Screen,
    scale: f32,
) {
    let radius = target
        .hurtbox
        .map_or(0.75, |hurtbox| hurtbox.height.min(2.0));
    let at = target.feet + Vec3::Y * radius;
    let Some(point) = (screen.project)(at) else {
        return;
    };
    let s = reticle_size(radius, at.distance(screen.eye)) * scale;
    let fade = clock.fade;
    let faded = |colour: Color| colour.with_alpha(colour.alpha() * fade);
    let ring = |texture, across: f32, degrees, colour| Sprite {
        texture,
        centre: point,
        size: Vec2::splat(across),
        degrees,
        colour: faded(colour),
    };
    // Each ring is 2s across: shrinking from twice that as it fades in, growing from
    // nothing against it, and fixed, tinted blue.
    out.push(ring(
        texture::LOCK_OUTER,
        2.0 * s * (2.0 - fade),
        clock.spin,
        ps2(0x80, 0x80, 0x80, 0x40),
    ));
    out.push(ring(
        texture::LOCK_INNER,
        2.0 * s * fade,
        -clock.spin,
        ps2(0x80, 0x80, 0x80, 0x40),
    ));
    out.push(ring(
        texture::LOCK_FIXED,
        2.0 * s,
        45.0,
        ps2(0x20, 0x20, 0xE0, 0x30),
    ));

    let Some(hitpoints) = target.hitpoints.filter(|hitpoints| hitpoints.max > 0.0) else {
        return;
    };
    let span = arc_span(hitpoints.max);
    let left = (hitpoints.current / hitpoints.max).clamp(0.0, 1.0);
    let band = 13.0 / 16.0 * s;
    let dot = (span * band / ARC_DOTS as f32).max(2.0 / 16.0 * s);
    for dot_index in 0..ARC_DOTS {
        let along = (dot_index as f32 + 0.5) / ARC_DOTS as f32;
        // Centred below the target (down the screen is +y), green from its left end over
        // the share of health left, red past it.
        let angle = FRAC_PI_2 + span * (0.5 - along);
        let colour = if along <= left {
            ps2(0x00, 0x80, 0x00, 0x60)
        } else {
            ps2(0x80, 0x00, 0x00, 0x60)
        };
        out.push(Sprite {
            texture: texture::DOT,
            centre: point + Vec2::new(ops::cos(angle), ops::sin(angle)) * band,
            size: Vec2::splat(dot),
            degrees: 0.0,
            colour: faded(colour),
        });
    }
}

/// The marks the gun in his hand puts on its targets, as its [`Mark`] says.
fn marks(
    clock: &SightsClock,
    aim: &Aim,
    screen: &Screen,
    seen: &dyn Fn(Entity) -> Option<SeenTarget>,
    scale: f32,
) -> Vec<Sprite> {
    let mark = |texture, target: SeenTarget, colour| {
        if !target.alive() {
            return None;
        }
        let at = target.raised();
        Some(Sprite {
            texture,
            centre: (screen.project)(at)?,
            size: Vec2::splat(mark_size(at.distance(screen.eye)) * scale),
            // Turning against its spin.
            degrees: -clock.mark_spin,
            colour,
        })
    };
    // 0xFF0FFF0F.
    let green = marker(0x0F, 0xFF, 0x0F, 0xFF);
    let Some((sight, level)) = aim.sight else {
        return Vec::new();
    };
    match sight.mark {
        Mark::None => Vec::new(),
        Mark::Target {
            or_lock,
            follow_only,
            outside_look,
            not_raised,
            ready_only,
        } => {
            let shown = (!follow_only || !aim.aiming())
                && (!outside_look || !aim.looking)
                && (!not_raised || !aim.raised)
                && (!ready_only || aim.sights.ready);
            let target = aim.sights.target.or(aim.lock.filter(|_| or_lock));
            target
                .filter(|_| shown)
                .and_then(seen)
                .and_then(|target| mark(texture::MARK, target, green))
                .into_iter()
                .collect()
        }
        Mark::Each { below_level } => {
            if level >= below_level || aim.lock.is_some() {
                return Vec::new();
            }
            aim.sights
                .marks
                .iter()
                .filter_map(|&target| mark(texture::MARK, seen(target)?, green))
                .collect()
        }
        Mark::Triangles { within_degrees } => {
            let edge = ops::cos(within_degrees.to_radians());
            let newest = aim.sights.marks.len().saturating_sub(1);
            aim.sights
                .marks
                .iter()
                .enumerate()
                .filter_map(|(index, &target)| {
                    let target = seen(target)?;
                    let towards = (target.raised() - screen.eye).normalize_or_zero();
                    if towards.dot(screen.forward) <= edge {
                        return None;
                    }
                    // 0x7F40FF40, the newest 0x7F2040FF.
                    let colour = if index == newest {
                        marker(0xFF, 0x40, 0x20, 0x7F)
                    } else {
                        marker(0x40, 0xFF, 0x40, 0x7F)
                    };
                    mark(texture::TRIANGLE, target, colour)
                })
                .collect()
        }
    }
}

/// The sights' images by their effect-texture numbers ([`texture`]).
#[derive(Resource, Debug, Default, Clone)]
pub struct SightImages(pub BTreeMap<u16, Handle<Image>>);

impl SightImages {
    /// The image for `texture`; Bevy's plain white one for a number without one.
    pub fn get(&self, texture: u16) -> Handle<Image> {
        self.0.get(&texture).cloned().unwrap_or_default()
    }
}

/// A player's pool of sprite nodes, kept on its [`PlayerHud`] root.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SightSprites(Vec<Entity>);

/// Paints the placeholder images, white so the sprites' colours tint them.
pub(super) fn paint_sight_images(mut commands: Commands, images: Option<ResMut<Assets<Image>>>) {
    let Some(mut images) = images else {
        commands.init_resource::<SightImages>();
        return;
    };
    let mut handles = BTreeMap::new();
    let textures = [
        texture::DOT,
        texture::TRIANGLE,
        texture::MARK,
        texture::LOCK_OUTER,
        texture::LOCK_INNER,
        texture::LOCK_FIXED,
        texture::PLAIN,
    ];
    for number in textures.into_iter().chain(texture::CROSSHAIRS) {
        handles.insert(
            number,
            images.add(paint(64, |p| white(shape(number, p, 2.0 / 64.0)))),
        );
    }
    handles.insert(texture::SCOPE, images.add(paint(256, scope_pixel)));
    commands.insert_resource(SightImages(handles));
}

/// Gives each new [`PlayerHud`] its pool of sprite nodes, beneath its other parts.
pub(super) fn add_sight_sprites(mut commands: Commands, huds: Query<Entity, Added<PlayerHud>>) {
    for root in &huds {
        let pool: Vec<Entity> = (0..POOL)
            .map(|_| {
                commands
                    .spawn((
                        Name::new("sight"),
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        // Stretched, as the game draws a texture over its sprite's size.
                        ImageNode {
                            image_mode: NodeImageMode::Stretch,
                            ..default()
                        },
                        UiTransform::default(),
                        Visibility::Hidden,
                    ))
                    .id()
            })
            .collect();
        commands
            .entity(root)
            .insert((SightsClock::default(), SightSprites(pool.clone())))
            .insert_children(0, &pool);
    }
}

type SpriteNode<'a> = (
    &'a mut Node,
    &'a mut ImageNode,
    &'a mut UiTransform,
    &'a mut Visibility,
);

/// Works out each player's sights from their hero and camera and puts them on screen.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_sights(
    time: Res<Time>,
    armory: Res<Armory>,
    images: Res<SightImages>,
    heroes: Query<(&Hero, &HeroBody, Option<&Arsenal>, Option<&LockOn>)>,
    cameras: Query<(&Camera, &GlobalTransform, &FollowCamera)>,
    targets: Query<(&GlobalTransform, Option<&Hurtbox>, Option<&Hitpoints>)>,
    enemies: Query<(&GlobalTransform, &Hurtbox, Option<&Hitpoints>), With<Hostile>>,
    mut huds: Query<(&PlayerHud, &mut SightsClock, &SightSprites)>,
    mut nodes: Query<SpriteNode>,
) {
    let frames = time.delta_secs() * FRAME_RATE;
    let no_sights = Sights::default();
    for (hud, mut clock, pool) in &mut huds {
        let hero = heroes.iter().find(|(hero, ..)| hero.player == hud.player);
        let camera = cameras
            .iter()
            .find(|(.., follow)| follow.player == hud.player);
        let mut shown = Vec::new();
        if let (Some((_, body, arsenal, lock)), Some((camera, place, follow))) = (hero, camera)
            && let Some(rect) = camera.logical_viewport_rect()
        {
            let motion = &body.0;
            let gun = arsenal.and_then(|arsenal| {
                let weapon = arsenal.hand.gun()?;
                Some((armory.get(weapon)?, arsenal.gun(weapon)?.level))
            });
            let crosshair = match (gun, arsenal.map(|arsenal| arsenal.hand)) {
                (Some((def, _)), _) => def.sight.crosshair.as_ref(),
                (None, Some(Hand::Wrench) | None) => armory.rules.wrench_sight.as_ref(),
                (None, Some(_)) => None,
            };
            let sight = gun.map(|(def, level)| (&def.sight, level));
            let eye = place.translation();
            let forward = place.forward().as_vec3();
            let on_target = sight
                .and_then(|(sight, _)| sight.scope_reach)
                .is_some_and(|reach| {
                    let live = enemies.iter().filter(|(.., hitpoints)| {
                        hitpoints.is_none_or(|hitpoints| hitpoints.current > 0.0)
                    });
                    ray_hits(
                        eye,
                        forward,
                        reach,
                        live.map(|(place, hurtbox, _)| {
                            (
                                place.translation() + Vec3::Y * hurtbox.height,
                                hurtbox.radius,
                            )
                        }),
                    )
                });
            let aim = Aim {
                sight,
                crosshair,
                sights: arsenal.map_or(&no_sights, |arsenal| &arsenal.sights),
                lock: lock.and_then(|lock| lock.target),
                looking: motion.state == HeroState::Look,
                look_ready: motion.look_ready,
                third_person: motion.camera_setting == CameraSetting::ThirdPerson,
                raised: motion.gun_arm.raised,
                weight: follow.target_weight(),
                on_target,
            };
            let project = |at: Vec3| {
                camera
                    .world_to_viewport(place, at)
                    .ok()
                    .map(|point| point + rect.min)
            };
            let screen = Screen {
                rect,
                eye,
                forward,
                project: &project,
            };
            let seen = |target: Entity| {
                let (place, hurtbox, hitpoints) = targets.get(target).ok()?;
                Some(SeenTarget {
                    feet: place.translation(),
                    hurtbox: hurtbox.copied(),
                    hitpoints: hitpoints.copied(),
                })
            };
            shown = sprites(&mut clock, &aim, &screen, &seen, frames);
        }

        for (index, &entity) in pool.0.iter().enumerate() {
            let Ok((mut node, mut image, mut transform, mut visibility)) = nodes.get_mut(entity)
            else {
                continue;
            };
            let Some(sprite) = shown.get(index) else {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            };
            visibility.set_if_neq(Visibility::Inherited);
            let corner = sprite.centre - sprite.size / 2.0;
            node.left = px(corner.x);
            node.top = px(corner.y);
            node.width = px(sprite.size.x);
            node.height = px(sprite.size.y);
            let handle = images.get(sprite.texture);
            if image.image != handle {
                image.image = handle;
            }
            image.color = sprite.colour;
            transform.rotation = Rot2::degrees(sprite.degrees);
        }
    }
}

/// A `size`² image, from `pixel` at each point, −1 to 1 across and down.
fn paint(size: u32, pixel: impl Fn(Vec2) -> [f32; 4]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / size as f32 * 2.0 - 1.0;
            data.extend(pixel(p).map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn white(alpha: f32) -> [f32; 4] {
    [1.0, 1.0, 1.0, alpha]
}

/// How much of a pixel `pixel` across lies inside a shape whose edge is `distance` away,
/// negative inside.
fn inside(distance: f32, pixel: f32) -> f32 {
    (0.5 - distance / pixel).clamp(0.0, 1.0)
}

/// Distance from `p` to a ring of `radius`, `width` thick.
fn ring(p: Vec2, radius: f32, width: f32) -> f32 {
    (p.length() - radius).abs() - width / 2.0
}

/// Distance from `p` to the line from `a` to `b`, `width` thick.
fn line(p: Vec2, a: Vec2, b: Vec2, width: f32) -> f32 {
    let along = (p - a).dot(b - a) / (b - a).length_squared();
    p.distance(a + (b - a) * along.clamp(0.0, 1.0)) - width / 2.0
}

/// `distance`, kept only in `count` arcs round the centre each `share` of their slice.
fn arcs(p: Vec2, distance: f32, count: f32, share: f32) -> f32 {
    let slice = (ops::atan2(p.y, p.x) / TAU * count).rem_euclid(1.0);
    if slice < share { distance } else { 1.0 }
}

/// Lines from `from` to `to` out from the centre, at `count` angles from `start` degrees.
fn spokes(p: Vec2, count: u32, start: f32, from: f32, to: f32, width: f32) -> f32 {
    (0..count)
        .map(|spoke| {
            let angle = (start + 360.0 / count as f32 * spoke as f32).to_radians();
            let way = Vec2::new(ops::cos(angle), ops::sin(angle));
            line(p, way * from, way * to, width)
        })
        .fold(f32::INFINITY, f32::min)
}

/// The placeholder for `texture` at `p`: its coverage, for a pixel `pixel` across.
fn shape(texture: u16, p: Vec2, pixel: f32) -> f32 {
    let distance = match texture {
        texture::DOT => return (1.0 - p.length()).clamp(0.0, 1.0).powi(2),
        texture::PLAIN => return 1.0,
        texture::TRIANGLE => {
            // Pointing down (+y).
            let edges = [
                (Vec2::new(-0.75, -0.55), Vec2::new(0.75, -0.55)),
                (Vec2::new(0.75, -0.55), Vec2::new(0.0, 0.75)),
                (Vec2::new(0.0, 0.75), Vec2::new(-0.75, -0.55)),
            ];
            edges
                .iter()
                .map(|&(a, b)| {
                    let edge = b - a;
                    (p - a).perp_dot(edge) / edge.length()
                })
                .fold(f32::NEG_INFINITY, f32::max)
        }
        texture::MARK => ring(p, 0.7, 0.12).min(spokes(p, 4, 0.0, 0.4, 0.7, 0.1)),
        texture::LOCK_OUTER => arcs(p, ring(p, 0.84, 0.12), 4.0, 0.7),
        texture::LOCK_INNER => ring(p, 0.64, 0.05).min(spokes(p, 4, 45.0, 0.46, 0.64, 0.07)),
        texture::LOCK_FIXED => {
            let square = p.x.abs().max(p.y.abs()) - 0.9;
            if p.x.abs() > 0.55 && p.y.abs() > 0.55 {
                square.abs() - 0.04
            } else {
                1.0
            }
        }
        texture::SCOPE_RING => arcs(p, ring(p, 0.8, 0.14), 8.0, 0.6),
        // The Rift Inducer's and the Plasma Coil's: a ring round a dot.
        0x21 => ring(p, 0.55, 0.08).min(p.length() - 0.1),
        // The wrench's: a cross with its middle open.
        0x25 => spokes(p, 4, 0.0, 0.3, 0.9, 0.12),
        // The Annihilator's and the Suck Cannon's: a ring crossed on its diagonals.
        0x44 => ring(p, 0.6, 0.07).min(spokes(p, 4, 45.0, 0.4, 0.9, 0.07)),
        // The N60's: four dots round one.
        0x56 => (0..4)
            .map(|dot| {
                let angle = (90.0 * dot as f32).to_radians();
                p.distance(Vec2::new(ops::cos(angle), ops::sin(angle)) * 0.6) - 0.12
            })
            .fold(p.length() - 0.08, f32::min),
        // The Disc Blade Gun's: brackets either side of a dot.
        0x5A => {
            let brackets = if p.x.abs() > 0.45 {
                ring(p, 0.8, 0.1)
            } else {
                1.0
            };
            brackets.min(p.length() - 0.1)
        }
        // The Shock Blaster's: bars above and below a dot, on their side once turned.
        0x5B => line(p, Vec2::new(-0.6, -0.75), Vec2::new(0.6, -0.75), 0.16)
            .min(line(p, Vec2::new(-0.6, 0.75), Vec2::new(0.6, 0.75), 0.16))
            .min(p.length() - 0.12),
        _ => ring(p, 0.6, 0.1),
    };
    inside(distance, pixel)
}

/// The scope's image at `p`: a clear lens with dotted lines through its centre, its bezel,
/// and the dark surround.
fn scope_pixel(p: Vec2) -> [f32; 4] {
    let pixel = 2.0 / 256.0;
    let lens = 0.92;
    let from_centre = p.length();
    let bezel = inside(ring(p, lens + 0.03, 0.06), pixel);
    let surround = inside(lens + 0.06 - from_centre, pixel);
    let dots = [p.x, p.y]
        .into_iter()
        .map(|across| {
            let other = if across == p.x { p.y } else { p.x };
            let nearest = (other / 0.08).round() * 0.08;
            Vec2::new(across, other - nearest).length() - 0.012
        })
        .fold(f32::INFINITY, f32::min);
    let dotted = if from_centre < lens {
        inside(dots, pixel) * 0.8
    } else {
        0.0
    };
    let over = |under: [f32; 4], top: [f32; 4], cover: f32| {
        let alpha = top[3] * cover;
        let mut out = [0.0; 4];
        for channel in 0..3 {
            out[channel] = top[channel] * alpha + under[channel] * (1.0 - alpha);
        }
        out[3] = alpha + under[3] * (1.0 - alpha);
        out
    };
    let clear = [0.0; 4];
    let mut colour = over(clear, SURROUND, surround);
    colour = over(colour, BEZEL, bezel);
    over(colour, BEZEL, dotted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rac_combat::WeaponId;

    const RECT: Rect = Rect {
        min: Vec2::ZERO,
        max: Vec2::new(512.0, 416.0),
    };

    /// Looking down −z from the origin; a point is drawn 10 px a unit off the centre.
    fn project(at: Vec3) -> Option<Vec2> {
        (at.z < 0.0).then(|| RECT.center() + Vec2::new(at.x, -at.y) * 10.0)
    }

    fn screen(project: &dyn Fn(Vec3) -> Option<Vec2>) -> Screen<'_> {
        Screen {
            rect: RECT,
            eye: Vec3::ZERO,
            forward: Vec3::NEG_Z,
            project,
        }
    }

    fn sight(name: &str) -> Sight {
        let armory = Armory::uya();
        let weapon: WeaponId = armory.find(name).unwrap();
        armory.get(weapon).unwrap().sight
    }

    fn aim<'a>(sight: Option<(&'a Sight, u8)>, sights: &'a Sights) -> Aim<'a> {
        Aim {
            sight,
            crosshair: sight.and_then(|(sight, _)| sight.crosshair.as_ref()),
            sights,
            lock: None,
            looking: false,
            look_ready: false,
            third_person: true,
            raised: false,
            weight: 0.0,
            on_target: false,
        }
    }

    /// Targets 1 u tall standing at these places, with 6 hit points, 3 left.
    fn targets(places: &[(Entity, Vec3)]) -> impl Fn(Entity) -> Option<SeenTarget> + '_ {
        move |entity| {
            places
                .iter()
                .find(|(target, _)| *target == entity)
                .map(|&(_, feet)| SeenTarget {
                    feet,
                    hurtbox: Some(Hurtbox {
                        radius: 0.5,
                        height: 1.0,
                    }),
                    hitpoints: Some(Hitpoints {
                        current: 3.0,
                        max: 6.0,
                    }),
                })
        }
    }

    fn textures(sprites: &[Sprite]) -> Vec<u16> {
        sprites.iter().map(|sprite| sprite.texture).collect()
    }

    #[test]
    fn the_reticle_and_marks_size_by_distance_as_the_game_does() {
        assert_eq!(reticle_size(1.0, 1000.0), 16.0);
        assert_eq!(reticle_size(2.0, 0.5), 128.0);
        assert!((reticle_size(1.0, 10.0) - 350.0 * 0.1f32.atan()).abs() < 1e-3);
        assert!((mark_size(0.0) - 36.0).abs() < 1e-4);
        assert!((mark_size(150.0) - 10.0).abs() < 1e-4);
        assert!((arc_span(6.0) - std::f32::consts::PI).abs() < 1e-5);
        let on = (0..62).filter(|&frame| blink_on(frame as f32)).count();
        assert_eq!(on, 30);
    }

    #[test]
    fn a_crosshair_fades_in_over_13_frames_pulses_and_turns_red_over_a_target() {
        let crosshair = sight("Shock Blaster").crosshair.unwrap();
        let alpha = |frames| crosshair_colour(&crosshair, frames, 0.0, 0.0).alpha();
        assert!((alpha(1.0) - 5.0 / 128.0).abs() < 1e-5);
        assert!(alpha(12.0) < 0.5);
        assert_eq!(alpha(13.0), 0.5);
        // At the pulse's start it is halfway from green to white.
        let start = crosshair_colour(&crosshair, 20.0, 0.0, 0.0).to_srgba();
        let white = crosshair_colour(&crosshair, 20.0, 90.0, 0.0).to_srgba();
        assert!(start.red < 0.7 && start.green > 0.99);
        assert!(white.red > 0.99 && white.blue > 0.99);
        // Red past 0.95, solid.
        let red = crosshair_colour(&crosshair, 1.0, 0.0, 0.96).to_srgba();
        assert_eq!((red.red, red.green, red.alpha), (1.0, 0.0, 0.75));
        assert!(crosshair_colour(&crosshair, 1.0, 0.0, 0.95).to_srgba().red < 0.9);
    }

    #[test]
    fn the_lock_on_reticle_fades_in_over_10_frames_and_the_old_target_fades_out_first() {
        let mut clock = SightsClock::default();
        let (a, b) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        );
        clock.step_reticle(Some(a), 1.0);
        assert_eq!(clock.reticle, Some(a));
        // Twice the spin while fading in.
        assert_eq!(clock.spin, 1.5);
        for _ in 0..9 {
            clock.step_reticle(Some(a), 1.0);
        }
        assert!((clock.fade - 1.0).abs() < 1e-5);
        clock.step_reticle(Some(a), 1.0);
        assert!((clock.spin - (10.0 * 1.5 + 0.75)).abs() < 1e-4);
        // Locked on to another, the reticle stays on the first until it has faded out.
        for _ in 0..9 {
            clock.step_reticle(Some(b), 1.0);
            assert_eq!(clock.reticle, Some(a));
        }
        clock.step_reticle(Some(b), 1.0);
        assert_eq!(clock.reticle, Some(b));
    }

    #[test]
    fn the_reticle_rings_a_locked_target_with_a_health_arc_green_over_what_is_left() {
        let target = Entity::from_raw_u32(1).unwrap();
        let places = [(target, Vec3::new(0.0, 0.0, -10.0))];
        let seen = targets(&places);
        let sights = Sights::default();
        let mut aim = aim(None, &sights);
        aim.lock = Some(target);
        let mut clock = SightsClock::default();
        let mut drawn = Vec::new();
        for _ in 0..10 {
            drawn = sprites(&mut clock, &aim, &screen(&project), &seen, 1.0);
        }
        let rings = &drawn[..3];
        assert_eq!(
            textures(rings),
            [
                texture::LOCK_OUTER,
                texture::LOCK_INNER,
                texture::LOCK_FIXED
            ]
        );
        // Raised by its 1 u height, faded fully in: each ring 2s across.
        let s = reticle_size(1.0, Vec3::new(0.0, 1.0, -10.0).length());
        assert_eq!(rings[0].centre, RECT.center() + Vec2::new(0.0, -10.0));
        for ring in rings {
            assert!((ring.size.x - 2.0 * s).abs() < 1e-3);
        }
        let dots = &drawn[3..];
        assert_eq!(dots.len(), ARC_DOTS);
        let green = dots
            .iter()
            .filter(|dot| dot.colour.to_srgba().green > 0.5)
            .count();
        assert_eq!(green, ARC_DOTS / 2);
        // Half round at 6 hit points: its ends level with the reticle's centre.
        assert!(dots.iter().all(|dot| dot.centre.y > rings[0].centre.y));
    }

    #[test]
    fn the_n60s_crosshair_shows_aiming_unlocked_and_its_mark_sits_on_the_lock_on_target() {
        let n60 = sight("N60 Storm");
        let target = Entity::from_raw_u32(1).unwrap();
        let places = [(target, Vec3::new(2.0, 0.0, -15.0))];
        let seen = targets(&places);
        let sights = Sights::default();
        let mut clock = SightsClock::default();

        // In the follow camera, no crosshair; with a lock-on, the mark on its target.
        let mut follow = aim(Some((&n60, 0)), &sights);
        follow.lock = Some(target);
        let drawn = sprites(&mut clock, &follow, &screen(&project), &seen, 1.0);
        let marks: Vec<_> = drawn
            .iter()
            .filter(|sprite| sprite.texture == texture::MARK)
            .collect();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].centre, RECT.center() + Vec2::new(20.0, -10.0));
        assert!(!textures(&drawn).contains(&0x56));

        // Aiming in the lock-strafe camera with no lock-on: the crosshair, and no mark.
        let mut strafe = aim(Some((&n60, 0)), &sights);
        strafe.third_person = false;
        let mut clock = SightsClock::default();
        let drawn = sprites(&mut clock, &strafe, &screen(&project), &seen, 1.0);
        assert_eq!(textures(&drawn), [0x56]);
        assert_eq!(drawn[0].centre, RECT.center());
        assert_eq!(drawn[0].size, Vec2::splat(30.0));
    }

    #[test]
    fn the_hydras_triangles_show_within_60_degrees_of_the_view_the_newest_red_orange() {
        let hydra = sight("Spitting Hydra");
        let [ahead, aside, newest] = [1, 2, 3].map(|n| Entity::from_raw_u32(n).unwrap());
        let places = [
            (ahead, Vec3::new(0.0, 0.0, -10.0)),
            // 70° off the view.
            (
                aside,
                Vec3::new(10.0 * 70f32.to_radians().tan(), 1.0, -10.0),
            ),
            (newest, Vec3::new(-3.0, 0.0, -10.0)),
        ];
        let seen = targets(&places);
        let sights = Sights {
            marks: vec![ahead, aside, newest],
            ..default()
        };
        let mut clock = SightsClock::default();
        let drawn = sprites(
            &mut clock,
            &aim(Some((&hydra, 0)), &sights),
            &screen(&project),
            &seen,
            1.0,
        );
        assert_eq!(textures(&drawn), [texture::TRIANGLE; 2]);
        assert!(drawn[0].colour.to_srgba().green > drawn[0].colour.to_srgba().red);
        assert!(drawn[1].colour.to_srgba().red > drawn[1].colour.to_srgba().green);
        assert_eq!(drawn[0].degrees, -2.0);
    }

    #[test]
    fn the_flux_scope_shows_in_the_ready_look_with_its_ring_blinking_over_an_enemy() {
        let flux = sight("Flux Rifle");
        let seen = |_| None;
        let sights = Sights {
            ready: true,
            ..default()
        };
        let mut look = aim(Some((&flux, 0)), &sights);
        look.looking = true;
        let mut clock = SightsClock::default();
        // Not yet ready, no scope.
        assert!(sprites(&mut clock, &look, &screen(&project), &seen, 1.0).is_empty());
        look.look_ready = true;
        let drawn = sprites(&mut clock, &look, &screen(&project), &seen, 1.0);
        // The lens as high as the screen, and its sides.
        assert_eq!(
            textures(&drawn),
            [texture::SCOPE, texture::PLAIN, texture::PLAIN]
        );
        assert_eq!(drawn[0].size, Vec2::splat(416.0));
        look.on_target = true;
        let blinks: Vec<bool> = (0..31)
            .map(|_| {
                let drawn = sprites(&mut clock, &look, &screen(&project), &seen, 1.0);
                textures(&drawn).contains(&texture::SCOPE_RING)
            })
            .collect();
        assert_eq!(blinks.iter().filter(|&&on| on).count(), 15);
        assert!(ray_hits(
            Vec3::ZERO,
            Vec3::NEG_Z,
            128.0,
            [(Vec3::new(0.4, 0.0, -100.0), 0.5)]
        ));
        assert!(!ray_hits(
            Vec3::ZERO,
            Vec3::NEG_Z,
            128.0,
            [(Vec3::new(0.0, 0.0, -130.0), 0.5)]
        ));
    }

    #[test]
    fn the_wrench_draws_its_sight_only_in_the_look_without_a_pulse() {
        let armory = Armory::uya();
        let wrench = armory.rules.wrench_sight.as_ref().unwrap();
        let sights = Sights::default();
        let mut look = aim(None, &sights);
        look.crosshair = Some(wrench);
        let mut clock = SightsClock::default();
        let seen = |_| None;
        assert!(sprites(&mut clock, &look, &screen(&project), &seen, 1.0).is_empty());
        look.looking = true;
        let drawn = sprites(&mut clock, &look, &screen(&project), &seen, 20.0);
        assert_eq!(textures(&drawn), [0x25]);
        let colour = drawn[0].colour.to_srgba();
        assert!(colour.blue > colour.red && colour.alpha == 0.5);
    }
}
