//! Reading messages in the simulation in a way rollback keeps right.
//!
//! Bevy keeps a `MessageReader`'s place in the system, out of reach of a snapshot. A
//! message written after its reader in a tick (a blast's `HurtHero`, read by the heroes
//! the next tick) is then lost when a rollback loads the next tick's snapshot, and one
//! written in the abandoned future is read in its place.
//!
//! A [`SimMessageReader`] reads only messages written since the last tick ended. What was
//! written after it in the last tick it reads first, from [`SimInbox`], which snapshots
//! keep. Message types it reads are registered with
//! [`SimMessageApp::add_sim_message`].

use bevy::{ecs::system::SystemParam, prelude::*};
use std::marker::PhantomData;

use crate::rollback::RollbackApp;

/// What each simulation reader of `M` has still to read from before the current tick.
/// Rolled back.
#[derive(Resource, Debug, Clone)]
pub struct SimInbox<M: Message + Clone> {
    readers: Vec<Inbox<M>>,
}

impl<M: Message + Clone> Default for SimInbox<M> {
    fn default() -> Self {
        Self {
            readers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Inbox<M> {
    /// The message count this reader has read up to.
    cursor: usize,
    /// Messages from before the current tick it hasn't read yet.
    late: Vec<M>,
}

impl<M> Default for Inbox<M> {
    fn default() -> Self {
        Self {
            cursor: 0,
            late: Vec::new(),
        }
    }
}

/// Where the current tick's `M` messages start, and how many readers there are. Not
/// rolled back: after a load, the next tick starts after everything the abandoned ticks
/// wrote.
#[derive(Resource, Debug)]
pub struct SimMessageMarks<M: Message> {
    tick_start: usize,
    last_end: usize,
    readers: usize,
    marker: PhantomData<M>,
}

impl<M: Message> Default for SimMessageMarks<M> {
    fn default() -> Self {
        Self {
            tick_start: 0,
            last_end: 0,
            readers: 0,
            marker: PhantomData,
        }
    }
}

/// How many `M` messages have ever been written.
fn written<M: Message>(messages: &Messages<M>) -> usize {
    messages.oldest_message_count() + messages.len()
}

/// The messages numbered `from..to` that are still kept.
fn between<M: Message + Clone>(messages: &Messages<M>, from: usize, to: usize) -> Vec<M> {
    (from.max(messages.oldest_message_count())..to)
        .filter_map(|id| messages.get_message(id).map(|(message, _)| message.clone()))
        .collect()
}

/// Reads `M` messages in a simulation system, like Bevy's `MessageReader` but safe across
/// rollbacks (see the module docs).
#[derive(SystemParam)]
pub struct SimMessageReader<'w, 's, M: Message + Clone> {
    messages: Res<'w, Messages<M>>,
    inbox: ResMut<'w, SimInbox<M>>,
    marks: ResMut<'w, SimMessageMarks<M>>,
    slot: Local<'s, Option<usize>>,
}

impl<M: Message + Clone> SimMessageReader<'_, '_, M> {
    /// Every message this reader hasn't read: those from before this tick first, then this
    /// tick's, in the order they were written.
    pub fn read(&mut self) -> std::vec::IntoIter<M> {
        let slot = *self.slot.get_or_insert_with(|| {
            self.marks.readers += 1;
            self.marks.readers - 1
        });
        if self.inbox.readers.len() <= slot {
            self.inbox.readers.resize_with(slot + 1, Inbox::default);
        }
        let now = written(&self.messages);
        let inbox = &mut self.inbox.readers[slot];
        let mut read = std::mem::take(&mut inbox.late);
        let from = inbox.cursor.max(self.marks.tick_start);
        read.extend(between(&self.messages, from, now));
        inbox.cursor = now;
        read.into_iter()
    }
}

/// Registers message types the simulation reads with [`SimMessageReader`].
pub trait SimMessageApp {
    /// Adds the message `M` (if it isn't yet) and keeps what simulation readers of it still
    /// have to read in every snapshot.
    fn add_sim_message<M: Message + Clone>(&mut self) -> &mut Self;
}

impl SimMessageApp for App {
    fn add_sim_message<M: Message + Clone>(&mut self) -> &mut Self {
        if self.world().contains_resource::<SimMessageMarks<M>>() {
            return self;
        }
        self.add_message::<M>()
            .init_resource::<SimInbox<M>>()
            .init_resource::<SimMessageMarks<M>>()
            .roll_back_resource::<SimInbox<M>>()
            .add_systems(FixedFirst, open_tick::<M>)
            .add_systems(FixedLast, close_tick::<M>)
    }
}

/// Starts this tick's messages where the last tick's ended, so what the app (or a test)
/// wrote between ticks is this tick's too. After a load that leaves out everything the
/// abandoned ticks wrote.
fn open_tick<M: Message + Clone>(mut marks: ResMut<SimMessageMarks<M>>) {
    marks.tick_start = marks.last_end;
}

/// Keeps, for each reader, what was written this tick after it read.
fn close_tick<M: Message + Clone>(
    messages: Res<Messages<M>>,
    mut marks: ResMut<SimMessageMarks<M>>,
    mut inbox: ResMut<SimInbox<M>>,
) {
    let now = written(&messages);
    let tick_start = marks.tick_start;
    for reader in &mut inbox.readers {
        let from = reader.cursor.max(tick_start);
        reader.late.extend(between(&messages, from, now));
        reader.cursor = now;
    }
    marks.last_end = now;
}
