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

use bevy::{ecs::system::SystemParam, prelude::*};
use std::collections::{BTreeMap, VecDeque};

use crate::SimTick;

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
    marker: std::marker::PhantomData<M>,
}

impl<M: Message> Default for TickRuns<M> {
    fn default() -> Self {
        Self {
            runs: VecDeque::new(),
            next_run: 0,
            marker: std::marker::PhantomData,
        }
    }
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
    log: Local<'s, ShownLog<M>>,
}

impl<M: Message + Clone + PartialEq> ShownReader<'_, '_, M> {
    /// The messages this reader hasn't read whose tick hasn't shown them yet, in the order
    /// they were written.
    pub fn read(&mut self) -> std::slice::Iter<'_, M> {
        let log = &mut *self.log;
        log.batch.clear();
        let mut newest = None;
        for (message, id) in self.reader.read_with_id() {
            let new = match self.runs.run_of(id.id) {
                Some((tick, run)) => {
                    newest = newest.max(Some(tick));
                    log.is_new(tick, run, message)
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
