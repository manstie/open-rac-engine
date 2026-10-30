//! The guns' sounds (PLAN.md P1.13): what a gun's events and the loops it holds sound like,
//! heard from the camera as UYA hears them (`docs/research/effects.md`, "Sounds"); and the
//! sounds of crates, pickups and the vendor's menu ([`PickupSound`], P1.10).
//!
//! - A [`Cue`] is what to play: one for each kind of [`GunEvent`], and the loops of the
//!   gun's [`Sustain`]. A gun's cues are named by its [`WeaponDef::effects`] key.
//! - The kit plays placeholders made from the recipes in `assets/sounds.ron`
//!   ([`SoundLayout`]) on its [`Mixer`], which Bevy hears as one [`SoundStream`]. A game
//!   with its own sounds claims the cues it plays ([`SoundClaims`]) and plays them itself,
//!   on its own stream; the placeholders leave those alone.
//! - A sound is at full volume within `near` of the nearest camera and falls evenly to
//!   nothing at `far`; its pan is its angle round that camera, 0 ahead and 90 to the right
//!   (UYA's `sound_volume` and `sound_pan`). [`Listeners`] holds the cameras for any
//!   game's sounds.
//! - A one-shot keeps the volume and pan it started with, as UYA's do. A loop follows the
//!   hero who holds it.
//!
//! [`WeaponDef::effects`]: rac_combat::WeaponDef

mod clips;
mod mixer;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use bevy::audio::{AddAudioSource, AudioPlugin};
use bevy::prelude::*;
use rac_combat::pickups::{PickupSound, PickupSoundKind};
use rac_combat::{Armory, Arsenal, GunEvent, GunEventKind, Sustain, WeaponId};
use rac_hero::Hero;
use serde::Deserialize;

pub use clips::{Recipe, Wave};
pub use mixer::{Clip, Mixer, Render, SAMPLE_RATE, SoundStream, StreamDecoder, Voice, gains};

/// The kit's sounds: the placeholder recipes and which gun plays which.
pub const SOUNDS_RON: &str = include_str!("../../assets/sounds.ron");

/// Where a held loop is heard from: the hero's middle, about where his gun is.
const HELD_AT: Vec3 = Vec3::new(0.0, 0.9, 0.0);

/// What a gun sounds like doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum Cue {
    /// A shot or a stream's round left the muzzle ([`GunEventKind::Fired`]).
    Fire,
    /// The trigger found no ammo.
    Empty,
    /// A shot hit, burst or splashed.
    Impact,
    /// A shot glanced off something.
    Bounce,
    /// The shock mod's arc struck.
    Arc,
    /// The gun went up a level.
    LevelUp,
    /// Held, it marked a target to strike.
    Mark,
    /// Let go too soon, it fizzled.
    Fizzle,
    /// Loops while the gun keeps it up ([`Sustain`]).
    Charge,
    Flow,
    Suck,
    ZoomIn,
    ZoomOut,
}

impl Cue {
    /// The cue of a gun's event, and where it is heard from (`None`: not from anywhere, at
    /// full volume). Rays are seen, not heard: their gun's `Fire` is.
    pub fn of_event(kind: GunEventKind) -> Option<(Cue, Option<Vec3>)> {
        match kind {
            GunEventKind::Fired { at, .. } => Some((Cue::Fire, Some(at))),
            GunEventKind::Empty { at } => Some((Cue::Empty, Some(at))),
            GunEventKind::Impact { at, .. } => Some((Cue::Impact, Some(at))),
            GunEventKind::Bounce { at } => Some((Cue::Bounce, Some(at))),
            GunEventKind::Arc { to, .. } => Some((Cue::Arc, Some(to))),
            GunEventKind::LevelUp { .. } => Some((Cue::LevelUp, None)),
            GunEventKind::Marked { at } => Some((Cue::Mark, Some(at))),
            GunEventKind::Fizzled { at } => Some((Cue::Fizzle, Some(at))),
            GunEventKind::Ray { .. } => None,
        }
    }

    /// The loop of what a gun keeps doing.
    pub fn held(sustain: Sustain) -> Cue {
        match sustain {
            Sustain::Charging => Cue::Charge,
            Sustain::Flowing => Cue::Flow,
            Sustain::Sucking => Cue::Suck,
            Sustain::ZoomingIn => Cue::ZoomIn,
            Sustain::ZoomingOut => Cue::ZoomOut,
        }
    }

    pub fn loops(self) -> bool {
        matches!(
            self,
            Cue::Charge | Cue::Flow | Cue::Suck | Cue::ZoomIn | Cue::ZoomOut
        )
    }
}

/// A gun's loop being held: whose, which gun at which level, the cue, and where from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldCue {
    pub player: usize,
    pub weapon: WeaponId,
    pub level: u8,
    pub cue: Cue,
    pub at: Vec3,
}

/// The loop each hero's gun in hand is holding, for any game's sounds to play.
pub fn held_cues<'a>(
    heroes: impl IntoIterator<Item = (&'a Hero, &'a Arsenal, &'a GlobalTransform)>,
) -> impl Iterator<Item = HeldCue> {
    heroes.into_iter().filter_map(|(hero, arsenal, place)| {
        let sustain = arsenal.sustain?;
        let weapon = arsenal.hand.gun()?;
        Some(HeldCue {
            player: hero.player,
            weapon,
            level: arsenal.gun(weapon).map_or(0, |owned| owned.level),
            cue: Cue::held(sustain),
            at: place.transform_point(HELD_AT),
        })
    })
}

/// The cues a game plays itself, by gun key, and the pickup sounds it plays; the kit's
/// placeholders skip them.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct SoundClaims {
    cues: BTreeSet<(String, Cue)>,
    pickups: BTreeSet<PickupSoundKind>,
}

impl SoundClaims {
    pub fn claim(&mut self, key: &str, cue: Cue) {
        self.cues.insert((key.to_owned(), cue));
    }

    pub fn claimed(&self, key: &str, cue: Cue) -> bool {
        self.cues.contains(&(key.to_owned(), cue))
    }

    pub fn claim_pickup(&mut self, kind: PickupSoundKind) {
        self.pickups.insert(kind);
    }

    pub fn pickup_claimed(&self, kind: PickupSoundKind) -> bool {
        self.pickups.contains(&kind)
    }
}

/// Where a sound is from the listener nearest it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    pub distance: f32,
    /// Degrees round the listener, from 0 ahead through 90 to the right, below 360.
    pub pan: f32,
}

/// The cameras sounds are heard from, this frame.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct Listeners(pub Vec<GlobalTransform>);

impl Listeners {
    /// How `at` is heard by the nearest listener; `None` without one.
    pub fn hear(&self, at: Vec3) -> Option<Heard> {
        let listener = self.0.iter().min_by(|a, b| {
            let a = a.translation().distance_squared(at);
            let b = b.translation().distance_squared(at);
            a.total_cmp(&b)
        })?;
        let local = listener.affine().inverse().transform_point3(at);
        // Cameras look down their −z, with +x to the right.
        let pan = ops::atan2(local.x, -local.z).to_degrees().rem_euclid(360.0);
        Some(Heard {
            distance: local.length(),
            pan: if pan.is_finite() { pan } else { 0.0 },
        })
    }
}

/// Full volume within `near`, falling evenly to nothing at `far` (1 is full).
pub fn falloff(distance: f32, near: f32, far: f32) -> f32 {
    if distance <= near {
        return 1.0;
    }
    if distance >= far {
        return 0.0;
    }
    (far - distance) / (far - near)
}

/// The kit's sounds, from `assets/sounds.ron`.
#[derive(Resource, Debug, Clone, PartialEq, Deserialize)]
pub struct SoundLayout {
    /// How far the placeholders are heard: fully within `near`, not at all from `far`.
    pub near: f32,
    pub far: f32,
    /// The placeholders, by name.
    pub clips: BTreeMap<String, Recipe>,
    /// What each cue plays for a gun that names none of its own.
    pub cues: BTreeMap<Cue, String>,
    /// Each gun's own, by its effects key; an empty name plays nothing.
    pub guns: BTreeMap<String, BTreeMap<Cue, String>>,
    /// What each pickup sound plays.
    #[serde(default)]
    pub pickups: BTreeMap<PickupSoundKind, String>,
}

impl Default for SoundLayout {
    fn default() -> Self {
        ron::from_str(SOUNDS_RON).expect("the kit's sounds.ron parses")
    }
}

impl SoundLayout {
    /// The placeholder `key`'s gun plays for `cue`, if any.
    pub fn clip(&self, key: &str, cue: Cue) -> Option<&str> {
        self.guns
            .get(key)
            .and_then(|cues| cues.get(&cue))
            .or_else(|| self.cues.get(&cue))
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }
}

/// The placeholders, made from their recipes.
#[derive(Resource, Debug, Default, Clone)]
pub struct Placeholders(pub BTreeMap<String, Clip>);

/// The kit's mixer, which its stream plays.
#[derive(Resource, Clone)]
pub struct KitMixer(pub Arc<Mutex<Mixer>>);

impl KitMixer {
    fn lock(&self) -> std::sync::MutexGuard<'_, Mixer> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The placeholder loops playing, by player.
#[derive(Resource, Debug, Default)]
struct HeldVoices(BTreeMap<usize, (String, Cue, Voice)>);

/// The guns' sounds. Without Bevy's [`AudioPlugin`] (as in tests) only the listeners are
/// kept. It registers [`SoundStream`] with Bevy's audio, so a game playing its own stream
/// mustn't again.
pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundLayout>()
            .init_resource::<SoundClaims>()
            .init_resource::<Listeners>()
            .add_systems(Update, hear_from_cameras.in_set(SfxSystems::Listen));
        if !app.is_plugin_added::<AudioPlugin>() {
            return;
        }
        app.add_audio_source::<SoundStream>()
            .insert_resource(KitMixer(Arc::new(Mutex::new(Mixer::default()))))
            .init_resource::<HeldVoices>()
            .configure_sets(Update, SfxSystems::Play.after(SfxSystems::Listen))
            .add_systems(Startup, start_placeholders)
            .add_systems(
                Update,
                (
                    play_placeholders,
                    play_pickup_placeholders,
                    hold_placeholders,
                )
                    .in_set(SfxSystems::Play),
            );
    }
}

/// When the sounds are worked out each frame: games playing their own put theirs in
/// [`SfxSystems::Play`] too, after the listeners have moved.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SfxSystems {
    Listen,
    Play,
}

fn hear_from_cameras(
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut listeners: ResMut<Listeners>,
) {
    listeners.0.clear();
    listeners.0.extend(
        cameras
            .iter()
            .filter(|(camera, _)| camera.is_active)
            .map(|(_, place)| *place),
    );
}

/// Makes the placeholders and starts the stream that plays them.
fn start_placeholders(
    layout: Res<SoundLayout>,
    mixer: Res<KitMixer>,
    mut streams: ResMut<Assets<SoundStream>>,
    mut commands: Commands,
) {
    let clips = layout
        .clips
        .iter()
        .map(|(name, recipe)| (name.clone(), recipe.render()))
        .collect();
    commands.insert_resource(Placeholders(clips));
    let stream = streams.add(SoundStream(mixer.0.clone()));
    commands.spawn((
        Name::new("placeholder sounds"),
        AudioPlayer(stream),
        PlaybackSettings::ONCE,
    ));
}

/// The volumes a placeholder starts at, heard from `at` (or from nowhere).
fn placeholder_gains(layout: &SoundLayout, listeners: &Listeners, at: Option<Vec3>) -> [f32; 2] {
    match at {
        None => gains(1.0, 0.0),
        Some(at) => match listeners.hear(at) {
            Some(heard) => gains(falloff(heard.distance, layout.near, layout.far), heard.pan),
            None => [0.0; 2],
        },
    }
}

fn play_placeholders(
    layout: Res<SoundLayout>,
    claims: Res<SoundClaims>,
    listeners: Res<Listeners>,
    placeholders: Res<Placeholders>,
    armory: Res<Armory>,
    mixer: Res<KitMixer>,
    mut events: MessageReader<GunEvent>,
) {
    let mut mixer = mixer.lock();
    for event in events.read() {
        let Some((cue, at)) = Cue::of_event(event.kind) else {
            continue;
        };
        let Some(def) = armory.get(event.weapon) else {
            continue;
        };
        if claims.claimed(&def.effects, cue) {
            continue;
        }
        let Some(clip) = layout
            .clip(&def.effects, cue)
            .and_then(|name| placeholders.0.get(name))
        else {
            continue;
        };
        let gains = placeholder_gains(&layout, &listeners, at);
        if gains != [0.0; 2] {
            mixer.play(clip.clone(), gains, false);
        }
    }
}

/// Plays the placeholder of each pickup sound no game has claimed.
fn play_pickup_placeholders(
    layout: Res<SoundLayout>,
    claims: Res<SoundClaims>,
    listeners: Res<Listeners>,
    placeholders: Res<Placeholders>,
    mixer: Res<KitMixer>,
    mut sounds: MessageReader<PickupSound>,
) {
    let mut mixer = mixer.lock();
    for sound in sounds.read() {
        if claims.pickup_claimed(sound.kind) {
            continue;
        }
        let Some(clip) = layout
            .pickups
            .get(&sound.kind)
            .and_then(|name| placeholders.0.get(name))
        else {
            continue;
        };
        let gains = placeholder_gains(&layout, &listeners, sound.at);
        if gains != [0.0; 2] {
            mixer.play(clip.clone(), gains, false);
        }
    }
}

/// Starts each hero's loop when his gun starts holding one, keeps it panned on him, and
/// stops it when he lets go.
fn hold_placeholders(
    layout: Res<SoundLayout>,
    claims: Res<SoundClaims>,
    listeners: Res<Listeners>,
    placeholders: Res<Placeholders>,
    armory: Res<Armory>,
    mixer: Res<KitMixer>,
    heroes: Query<(&Hero, &Arsenal, &GlobalTransform)>,
    mut held: ResMut<HeldVoices>,
) {
    let mut mixer = mixer.lock();
    let mut wanted = BTreeMap::new();
    for cue in held_cues(&heroes) {
        let Some(def) = armory.get(cue.weapon) else {
            continue;
        };
        if !claims.claimed(&def.effects, cue.cue) {
            wanted.insert(cue.player, (def.effects.clone(), cue.cue, cue.at));
        }
    }
    held.0.retain(|player, (key, cue, voice)| {
        let keep = wanted
            .get(player)
            .is_some_and(|(want_key, want_cue, _)| want_key == key && want_cue == cue);
        if !keep {
            mixer.stop(*voice);
        }
        keep
    });
    for (player, (key, cue, at)) in wanted {
        let gains = placeholder_gains(&layout, &listeners, Some(at));
        if let Some((_, _, voice)) = held.0.get(&player) {
            mixer.set_gains(*voice, gains);
            continue;
        }
        let Some(clip) = layout
            .clip(&key, cue)
            .and_then(|name| placeholders.0.get(name))
        else {
            continue;
        };
        let voice = mixer.play(clip.clone(), gains, true);
        held.0.insert(player, (key, cue, voice));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_and_cue_names_a_placeholder_that_has_a_recipe() {
        let layout = SoundLayout::default();
        let armory = Armory::uya();
        for def in &armory.weapons {
            assert!(
                layout.guns.contains_key(&def.effects),
                "{} has no sounds",
                def.effects
            );
            // The Lava Gun's stream is heard as its loop.
            let fire = layout.clip(&def.effects, Cue::Fire);
            assert!(fire.or(layout.clip(&def.effects, Cue::Flow)).is_some());
        }
        // Every pickup sound has one.
        for kind in PickupSoundKind::ALL {
            assert!(layout.pickups.contains_key(&kind), "{kind:?} has no sound");
        }
        let named = layout
            .cues
            .values()
            .chain(layout.guns.values().flat_map(|cues| cues.values()))
            .chain(layout.pickups.values());
        for name in named.filter(|name| !name.is_empty()) {
            assert!(layout.clips.contains_key(name), "no recipe for {name}");
        }
        for (name, recipe) in &layout.clips {
            let clip = recipe.render();
            assert!(!clip.is_empty(), "{name}");
            assert!(
                clip.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name}"
            );
        }
    }

    #[test]
    fn a_sound_is_heard_by_the_nearest_camera_round_its_way() {
        let camera = GlobalTransform::from(
            Transform::from_xyz(0.0, 0.0, 0.0).looking_to(Vec3::NEG_Z, Vec3::Y),
        );
        let far = GlobalTransform::from(Transform::from_xyz(100.0, 0.0, 0.0));
        let listeners = Listeners(vec![far, camera]);
        let ahead = listeners.hear(Vec3::new(0.0, 0.0, -10.0)).unwrap();
        assert!((ahead.distance - 10.0).abs() < 1e-4 && ahead.pan.abs() < 1e-3);
        let right = listeners.hear(Vec3::new(5.0, 0.0, 0.0)).unwrap();
        assert!((right.pan - 90.0).abs() < 1e-3);
        let left = listeners.hear(Vec3::new(-5.0, 0.0, 0.0)).unwrap();
        assert!((left.pan - 270.0).abs() < 1e-3);
        assert_eq!(falloff(0.0, 0.0, 40.0), 1.0);
        assert_eq!(falloff(30.0, 0.0, 40.0), 0.25);
        assert_eq!(falloff(50.0, 0.0, 40.0), 0.0);
    }

    #[test]
    fn a_guns_event_plays_its_placeholder_unless_the_game_claims_it() {
        let armory = Armory::uya();
        let n60 = WeaponId(
            armory
                .weapons
                .iter()
                .position(|def| def.effects == "n60")
                .unwrap(),
        );
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<GunEvent>()
            .insert_resource(armory)
            .init_resource::<SoundLayout>()
            .init_resource::<SoundClaims>()
            .insert_resource(Listeners(vec![GlobalTransform::IDENTITY]))
            .insert_resource(KitMixer(Arc::new(Mutex::new(Mixer::default()))))
            .add_systems(Update, play_placeholders);
        let layout = app.world().resource::<SoundLayout>().clone();
        app.insert_resource(Placeholders(
            layout
                .clips
                .iter()
                .map(|(name, recipe)| (name.clone(), recipe.render()))
                .collect(),
        ));
        let fired = |at: Vec3| GunEvent {
            player: 0,
            weapon: n60,
            kind: GunEventKind::Fired {
                at,
                direction: Vec3::NEG_Z,
            },
            shot: None,
        };
        let voices = |app: &App| app.world().resource::<KitMixer>().lock().voices();
        app.world_mut()
            .write_message(fired(Vec3::new(0.0, 0.0, -5.0)));
        app.update();
        assert_eq!(voices(&app), 1);
        // Out of earshot it isn't started.
        app.world_mut()
            .write_message(fired(Vec3::new(0.0, 0.0, -500.0)));
        app.update();
        assert_eq!(voices(&app), 1);
        app.world_mut()
            .resource_mut::<SoundClaims>()
            .claim("n60", Cue::Fire);
        app.world_mut()
            .write_message(fired(Vec3::new(0.0, 0.0, -5.0)));
        app.update();
        assert_eq!(voices(&app), 1, "the game plays it");
    }

    #[test]
    fn claimed_cues_are_left_to_the_game() {
        let mut claims = SoundClaims::default();
        claims.claim("n60", Cue::Fire);
        assert!(claims.claimed("n60", Cue::Fire));
        assert!(!claims.claimed("n60", Cue::Empty));
        assert!(!claims.claimed("lava", Cue::Fire));
    }
}
