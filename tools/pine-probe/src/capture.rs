//! Per-frame capture of memory regions from a running PCSX2.
//!
//! PINE reads run on PCSX2's own thread while the emulated CPU keeps going, so any sample
//! can catch the game halfway through updating the hero. We poll as fast as PINE answers
//! (about 20,000 samples a second on the owner's machine, so hundreds per frame) and:
//! 1. read the frame counter before and after the regions, and drop the sample if it
//!    changed in between;
//! 2. group samples by frame-counter value;
//! 3. within a frame, pick the version of the layout's key fields seen most often. The
//!    game writes those fields in a short burst and then leaves them alone for the rest of
//!    the frame, so the most common version is the settled one.

use std::time::{Duration, Instant};

use crate::layout::Layout;
use crate::pine::{Batch, Pine, PineError};
use crate::trace::{Record, TraceError, TraceWriter};

/// Chooses one sample per frame; see the module documentation.
#[derive(Default)]
pub struct FrameVoter {
    frame: Option<u32>,
    /// `(key, votes, time of last sighting, latest data with that key)`.
    candidates: Vec<(Vec<u8>, u32, f64, Vec<u8>)>,
}

impl FrameVoter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one consistent sample. Returns the previous frame's winner when `frame`
    /// differs from the frame being collected.
    pub fn push(&mut self, frame: u32, time: f64, key: Vec<u8>, data: &[u8]) -> Option<Record> {
        let finished = if self.frame != Some(frame) {
            let done = self.finish();
            self.frame = Some(frame);
            done
        } else {
            None
        };
        match self.candidates.iter_mut().find(|c| c.0 == key) {
            Some(c) => {
                c.1 += 1;
                c.2 = time;
                c.3.clear();
                c.3.extend_from_slice(data);
            }
            None => self.candidates.push((key, 1, time, data.to_vec())),
        }
        finished
    }

    /// Returns the winner for the frame being collected, if any, and resets.
    pub fn finish(&mut self) -> Option<Record> {
        let frame = self.frame.take()?;
        let best = self
            .candidates
            .drain(..)
            .max_by_key(|c| c.1)
            .expect("a frame always has at least one sample");
        self.candidates.clear();
        Some(Record {
            frame,
            time: best.2,
            data: best.3,
        })
    }
}

/// When to stop capturing. Any condition that is set ends the capture.
#[derive(Debug, Clone, Default)]
pub struct StopWhen {
    /// After this many frames have been written.
    pub frames: Option<u64>,
    /// After this much wall-clock time.
    pub seconds: Option<f64>,
    /// When the frame counter has not moved for this long, but only once it has moved at
    /// least once. This ends a capture when PCSX2 pauses at the end of an input recording,
    /// while still allowing the capture to start before the game is unpaused.
    pub idle_seconds: Option<f64>,
}

#[derive(Debug, Default, Clone)]
pub struct CaptureStats {
    pub samples: u64,
    pub torn: u64,
    pub frames: u64,
    /// Frame-counter steps other than +1 between written frames.
    pub gaps: u64,
}

/// Reads the layout's regions every frame and writes the chosen samples to `writer`.
pub fn capture<W: std::io::Write>(
    pine: &mut Pine,
    layout: &Layout,
    writer: &mut TraceWriter<W>,
    stop: &StopWhen,
) -> Result<CaptureStats, CaptureError> {
    let mut batch = Batch::new();
    let before = batch.read_u32(layout.frame_counter);
    let regions: Vec<_> = layout
        .regions
        .iter()
        .map(|r| batch.read_bytes(r.base, r.size))
        .collect();
    let after = batch.read_u32(layout.frame_counter);
    let key_ranges = layout.key_ranges();

    let start = Instant::now();
    let mut stats = CaptureStats::default();
    let mut voter = FrameVoter::new();
    let mut sample = Vec::with_capacity(layout.sample_len());
    let mut first_frame = None;
    let mut last_written: Option<u32> = None;
    let mut last_change = Instant::now();
    let mut current = None;

    let mut write = |record: Record, stats: &mut CaptureStats| -> Result<(), CaptureError> {
        if let Some(prev) = last_written
            && record.frame != prev.wrapping_add(1)
        {
            stats.gaps += 1;
        }
        last_written = Some(record.frame);
        writer.write(&record)?;
        stats.frames += 1;
        Ok(())
    };

    loop {
        let elapsed = start.elapsed();
        if stop.seconds.is_some_and(|s| elapsed.as_secs_f64() >= s)
            || stop.frames.is_some_and(|n| stats.frames >= n)
        {
            break;
        }
        if let Some(idle) = stop.idle_seconds
            && current.is_some_and(|c| Some(c) != first_frame)
            && last_change.elapsed() >= Duration::from_secs_f64(idle)
        {
            break;
        }

        let reply = match pine.execute(&batch) {
            Ok(reply) => reply,
            // PCSX2 shutting down or the game stopping ends the capture cleanly.
            Err(PineError::Io(_) | PineError::Rejected) => break,
            Err(e) => return Err(e.into()),
        };
        stats.samples += 1;
        let word =
            |r: &std::ops::Range<usize>| u32::from_le_bytes(reply[r.clone()].try_into().unwrap());
        let frame = word(&before);
        if frame != word(&after) {
            stats.torn += 1;
            continue;
        }
        if current != Some(frame) {
            current = Some(frame);
            first_frame.get_or_insert(frame);
            last_change = Instant::now();
        }
        sample.clear();
        for r in &regions {
            sample.extend_from_slice(&reply[r.clone()]);
        }
        let key = if key_ranges.is_empty() {
            sample.clone()
        } else {
            key_ranges
                .iter()
                .flat_map(|r| &sample[r.clone()])
                .copied()
                .collect()
        };
        if let Some(record) = voter.push(frame, elapsed.as_secs_f64(), key, &sample) {
            write(record, &mut stats)?;
        }
    }
    if let Some(record) = voter.finish()
        && stop.frames.is_none_or(|n| stats.frames < n)
    {
        write(record, &mut stats)?;
    }
    Ok(stats)
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error(transparent)]
    Pine(#[from] PineError),
    #[error(transparent)]
    Trace(#[from] TraceError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pine::tests::FakePcsx2;
    use crate::trace::{TraceHeader, TraceReader};

    #[test]
    fn voter_prefers_the_settled_sample() {
        let mut v = FrameVoter::new();
        assert!(v.push(10, 0.0, vec![1], &[1, 0]).is_none());
        // A torn read mid-update, then the settled value seen several times.
        assert!(v.push(10, 0.1, vec![2], &[2, 0]).is_none());
        for i in 0..3 {
            assert!(
                v.push(10, 0.2 + f64::from(i), vec![3], &[3, i as u8])
                    .is_none()
            );
        }
        let won = v.push(11, 5.0, vec![9], &[9, 9]).unwrap();
        assert_eq!(won.frame, 10);
        assert_eq!(won.data, [3, 2], "latest data with the winning key");
        let last = v.finish().unwrap();
        assert_eq!((last.frame, last.data), (11, vec![9, 9]));
        assert!(v.finish().is_none());
    }

    #[test]
    fn captures_until_the_frame_budget() {
        let mut memory = vec![0u8; 0x400];
        memory[0x10..0x14].copy_from_slice(&5u32.to_le_bytes());
        let fake = FakePcsx2::new(memory);
        let shared = fake.memory.clone();
        let layout = Layout::from_toml(
            r#"
            release = "SCES-00000"
            title = "Test"
            refresh_hz = 50
            frame_counter = 0x10
            [[region]]
            name = "hero"
            base = 0x100
            size = 0x20
            [[field]]
            name = "x"
            region = "hero"
            offset = 0
            type = "f32"
            key = true
            "#,
        )
        .unwrap();

        // Advance the "game" from another thread: bump the counter and the hero field.
        let ticker = std::thread::spawn(move || {
            for frame in 6..10u32 {
                std::thread::sleep(Duration::from_millis(20));
                let mut m = shared.lock().unwrap();
                m[0x100..0x104].copy_from_slice(&(frame as f32).to_le_bytes());
                m[0x10..0x14].copy_from_slice(&frame.to_le_bytes());
            }
        });
        let mut pine = Pine::from_stream(fake);
        let mut writer = TraceWriter::new(Vec::new(), &TraceHeader::from_layout(&layout)).unwrap();
        let stop = StopWhen {
            frames: Some(4),
            seconds: Some(5.0),
            ..StopWhen::default()
        };
        let stats = capture(&mut pine, &layout, &mut writer, &stop).unwrap();
        ticker.join().unwrap();
        assert_eq!(stats.frames, 4);
        assert_eq!(stats.gaps, 0);

        let bytes = writer.finish().unwrap();
        let mut reader = TraceReader::new(bytes.as_slice()).unwrap();
        let mut frames = Vec::new();
        while let Some(r) = reader.next_record().unwrap() {
            let x = f32::from_le_bytes(r.data[..4].try_into().unwrap());
            frames.push((r.frame, x));
        }
        assert_eq!(frames, [(5, 0.0), (6, 6.0), (7, 7.0), (8, 8.0)]);
    }
}
