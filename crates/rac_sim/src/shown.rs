//! Showing what the simulation says once, though rollback runs its ticks again
//! (`docs/netcode.md`, "Presentation of resimulated ticks").
//!
//! Effects, sounds and the HUD read messages the simulation writes (a shot fired, a crate
//! broken). After a rollback the ticks run again and write those messages again: read
//! plainly, a shot would sound twice. Nor can resimulated ticks' messages simply be left
//! out, since a shot the other player fired is first known when a rollback runs the tick
//! with their real input.
//!
//! A [`ShownReader`] reads each message the first time any run of its tick writes it. It
//! knows a message's tick and run from where each run's messages start, which
//! [`ShownMessageApp::add_shown_message`] notes as every run begins, and matches it against
//! what earlier runs of the tick showed. A run that writes what an earlier one did shows
//! nothing new; one that writes something else shows it, a few ticks late. What a run
//! showed and a later one didn't write is not taken back.
//!
//! A message naming a simulated entity (a target destroyed) is matched by the entity's
//! [`SimId`] ([`ShownMessageApp::add_shown_message_naming`]): a rollback that reaches back
//! past the entity's spawn spawns it again, under the same number but another `Entity`.

use bevy::{
    ecs::{
        entity::{EntityMapper, MapEntities},
        entity_disabling::Disabled,
        query::Allow,
        system::SystemParam,
    },
    prelude::*,
};
use std::collections::{BTreeMap, VecDeque};

use crate::{Departed, SimId, SimTick};

/// How many runs of ticks back a message's run is still known.
const KEPT_RUNS: usize = 512;
/// How many ticks back a reader remembers what it showed: more than a rollback reaches.
const KEPT_TICKS: u64 = 64;

/// Where each recent run of a tick began among the `M` messages. Not rolled back.
#[derive(Resource, Debug)]
pub struct TickRuns<M: Message> {
    /// The first message's number, the tick, and the run's number.
    runs: VecDeque<(usize, u64, u64)>,
    next_run: u64,
    /// Puts the numbers of the entities a message names in place of their `Entity`, for
    /// messages that name some.
    by_number: Option<fn(&mut M, &mut ByNumber<'_>)>,
}

impl<M: Message> Default for TickRuns<M> {
    fn default() -> Self {
        Self {
            runs: VecDeque::new(),
            next_run: 0,
            by_number: None,
        }
    }
}

/// Maps each simulated entity to an `Entity` standing for its [`SimId`], which every run
/// of a tick agrees on; other entities stay as they are.
struct ByNumber<'a>(&'a dyn Fn(Entity) -> Option<SimId>);

impl EntityMapper for ByNumber<'_> {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        (self.0)(source)
            .filter(|id| id.get() != 0)
            // The last generation, which no live entity reaches.
            .and_then(|id| Entity::try_from_bits(u64::from(id.get()) | u64::from(u32::MAX) << 32))
            .unwrap_or(source)
    }

    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

fn map_by_number<M: MapEntities>(message: &mut M, numbers: &mut ByNumber<'_>) {
    message.map_entities(numbers);
}

impl<M: Message> TickRuns<M> {
    /// The tick and run that wrote message `id`, if it was written in one still known.
    fn run_of(&self, id: usize) -> Option<(u64, u64)> {
        let at = self.runs.partition_point(|&(first, ..)| first <= id);
        let &(_, tick, run) = self.runs.get(at.checked_sub(1)?)?;
        Some((tick, run))
    }
}

/// Notes where this run of the tick begins among the `M` messages.
fn open_run<M: Message>(
    tick: Res<SimTick>,
    messages: Res<Messages<M>>,
    mut runs: ResMut<TickRuns<M>>,
) {
    let first = messages.oldest_message_count() + messages.len();
    let run = runs.next_run;
    runs.next_run += 1;
    runs.runs.push_back((first, tick.0, run));
    if runs.runs.len() > KEPT_RUNS {
        runs.runs.pop_front();
    }
}

/// Registers message types presentation reads with [`ShownReader`].
pub trait ShownMessageApp {
    /// Adds the message `M` (if it isn't yet) and notes where each run of a tick begins
    /// among its messages.
    fn add_shown_message<M: Message + Clone + PartialEq>(&mut self) -> &mut Self;

    /// Adds `M` as [`Self::add_shown_message`] does, for a message that names simulated
    /// entities: runs of a tick match it by the entities' [`SimId`]s, not their `Entity`
    /// values, which differ once a rollback spawns an entity again.
    fn add_shown_message_naming<M: Message + Clone + PartialEq + MapEntities>(
        &mut self,
    ) -> &mut Self;
}

impl ShownMessageApp for App {
    fn add_shown_message<M: Message + Clone + PartialEq>(&mut self) -> &mut Self {
        if self.world().contains_resource::<TickRuns<M>>() {
            return self;
        }
        if !self.world().contains_resource::<Messages<M>>() {
            self.add_message::<M>();
        }
        self.init_resource::<TickRuns<M>>()
            .add_systems(FixedFirst, open_run::<M>)
    }

    fn add_shown_message_naming<M: Message + Clone + PartialEq + MapEntities>(
        &mut self,
    ) -> &mut Self {
        self.add_shown_message::<M>();
        self.world_mut().resource_mut::<TickRuns<M>>().by_number = Some(map_by_number::<M>);
        self
    }
}

/// What one reader has shown.
#[derive(Debug)]
pub struct ShownLog<M> {
    /// Each recent tick's messages shown, in the order they were.
    ticks: BTreeMap<u64, Vec<M>>,
    /// The run being read, and which of its tick's messages shown before it began it has
    /// matched, so two alike in one run are both shown.
    run: Option<(u64, Vec<bool>)>,
    /// What the last read showed.
    batch: Vec<M>,
}

impl<M> Default for ShownLog<M> {
    fn default() -> Self {
        Self {
            ticks: BTreeMap::new(),
            run: None,
            batch: Vec::new(),
        }
    }
}

impl<M: Clone + PartialEq> ShownLog<M> {
    /// Whether `message`, from `run` of `tick`, is new: no earlier run of the tick showed
    /// it. A new one is remembered.
    fn is_new(&mut self, tick: u64, run: u64, message: &M) -> bool {
        let shown = self.ticks.entry(tick).or_default();
        if self.run.as_ref().is_none_or(|(reading, _)| *reading != run) {
            self.run = Some((run, vec![false; shown.len()]));
        }
        let Some((_, matched)) = &mut self.run else {
            return true;
        };
        // Only what earlier runs showed can match: this run's own come after the flags.
        let earlier = (0..matched.len()).find(|&i| !matched[i] && shown[i] == *message);
        match earlier {
            Some(i) => {
                matched[i] = true;
                false
            }
            None => {
                shown.push(message.clone());
                true
            }
        }
    }

    fn forget_before(&mut self, tick: u64) {
        self.ticks = self.ticks.split_off(&tick);
    }
}

/// Reads `M` messages for presentation, like Bevy's `MessageReader`, but each only once
/// however many times rollback runs its tick (see the module docs). Without rollback it
/// reads every message, as `MessageReader` does.
#[derive(SystemParam)]
pub struct ShownReader<'w, 's, M: Message + Clone + PartialEq> {
    reader: MessageReader<'w, 's, M>,
    runs: Res<'w, TickRuns<M>>,
    numbers: Query<'w, 's, &'static SimId, Allow<Disabled>>,
    departed: Option<Res<'w, Departed>>,
    log: Local<'s, ShownLog<M>>,
}

impl<M: Message + Clone + PartialEq> ShownReader<'_, '_, M> {
    /// The messages this reader hasn't read whose tick hasn't shown them yet, in the order
    /// they were written.
    pub fn read(&mut self) -> std::slice::Iter<'_, M> {
        let Self {
            reader,
            runs,
            numbers,
            departed,
            log,
        } = self;
        let log = &mut **log;
        let number = |entity| {
            numbers.get(entity).ok().copied().or_else(|| {
                departed
                    .as_ref()
                    .and_then(|departed| departed.number_of(entity))
            })
        };
        log.batch.clear();
        let mut newest = None;
        for (message, id) in reader.read_with_id() {
            let new = match runs.run_of(id.id) {
                Some((tick, run)) => {
                    newest = newest.max(Some(tick));
                    match runs.by_number {
                        Some(by_number) => {
                            let mut key = message.clone();
                            by_number(&mut key, &mut ByNumber(&number));
                            log.is_new(tick, run, &key)
                        }
                        None => log.is_new(tick, run, message),
                    }
                }
                // Written before any tick ran.
                None => true,
            };
            if new {
                log.batch.push(message.clone());
            }
        }
        if let Some(newest) = newest {
            log.forget_before(newest.saturating_sub(KEPT_TICKS));
        }
        log.batch.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimPlugin, Snapshot};
    use bevy::app::FixedMain;

    #[derive(Message, Debug, Clone, Copy, PartialEq)]
    struct Bang(u32);

    /// What the simulation writes at a tick: `extra` adds a bang only this run has.
    #[derive(Resource, Default)]
    struct Extra(bool);

    fn bang(tick: Res<SimTick>, extra: Res<Extra>, mut bangs: MessageWriter<Bang>) {
        if tick.0.is_multiple_of(2) {
            // Two alike in one tick, both shown.
            bangs.write(Bang(tick.0 as u32));
            bangs.write(Bang(tick.0 as u32));
        }
        if extra.0 {
            bangs.write(Bang(1000 + tick.0 as u32));
        }
    }

    #[derive(Resource, Default)]
    struct Heard(Vec<u32>);

    fn hear(mut bangs: ShownReader<Bang>, mut heard: ResMut<Heard>) {
        heard.0.extend(bangs.read().map(|bang| bang.0));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin))
            .add_shown_message::<Bang>()
            .init_resource::<Extra>()
            .init_resource::<Heard>()
            .add_systems(FixedUpdate, bang)
            .add_systems(Update, hear);
        app
    }

    /// Something simulated, spawned at tick 1.
    #[derive(Component)]
    #[require(SimId)]
    struct Thing;

    /// A message naming it, written at tick 2.
    #[derive(Message, MapEntities, Debug, Clone, Copy, PartialEq)]
    struct Named(#[entities] Entity);

    fn spawn_thing(tick: Res<SimTick>, mut commands: Commands) {
        if tick.0 == 1 {
            commands.spawn(Thing);
        }
    }

    fn name_things(
        tick: Res<SimTick>,
        things: Query<Entity, With<Thing>>,
        mut named: MessageWriter<Named>,
    ) {
        if tick.0 == 2 {
            named.write_batch(things.iter().map(Named));
        }
    }

    #[derive(Resource, Default)]
    struct HeardNamed(Vec<Entity>);

    fn hear_named(mut named: ShownReader<Named>, mut heard: ResMut<HeardNamed>) {
        heard.0.extend(named.read().map(|named| named.0));
    }

    /// Runs ticks 0 to 3, then 0 to 3 again from tick 0's snapshot, which spawns the thing
    /// again under another `Entity`, reading after each run or, `late`, only at the end,
    /// when the first thing has gone; returns what was heard.
    fn named_twice(naming: bool, late: bool) -> Vec<Entity> {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SimPlugin))
            .init_resource::<HeardNamed>()
            .add_systems(FixedUpdate, (spawn_thing, name_things))
            .add_systems(Update, hear_named);
        if naming {
            app.add_shown_message_naming::<Named>();
        } else {
            app.add_shown_message::<Named>();
        }
        let saved = Snapshot::save(app.world_mut());
        for run in 0..2 {
            if run > 0 {
                saved.load(app.world_mut());
            }
            for _ in 0..4 {
                app.world_mut().run_schedule(FixedMain);
            }
            if !late || run > 0 {
                app.world_mut().run_schedule(Update);
            }
        }
        app.world().resource::<HeardNamed>().0.clone()
    }

    #[test]
    fn an_entity_spawned_again_is_named_once() {
        // Matched by `Entity`, the thing spawned again is another.
        let plain = named_twice(false, false);
        assert_eq!(plain.len(), 2);
        assert_ne!(plain[0], plain[1]);
        // Matched by number, it is the same, whether the first is still there or gone.
        assert_eq!(named_twice(true, false), plain[..1]);
        assert_eq!(named_twice(true, true).len(), 1);
    }

    #[test]
    fn ticks_run_again_show_only_what_they_write_anew() {
        let mut app = app();
        let tick = |app: &mut App| app.world_mut().run_schedule(FixedMain);
        tick(&mut app);
        let saved = Snapshot::save(app.world_mut());
        for _ in 0..3 {
            tick(&mut app);
        }
        // Ticks 0 to 3 heard, then ticks 1 to 3 run again with a bang only they have.
        saved.load(app.world_mut());
        app.world_mut().resource_mut::<Extra>().0 = true;
        for _ in 0..3 {
            tick(&mut app);
        }
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world().resource::<Heard>().0,
            vec![0, 0, 2, 2, 1001, 1002, 1003]
        );
    }
}
