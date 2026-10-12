//! The trace file `pine-probe capture` writes: one record per game frame.
//!
//! Format (all little-endian):
//! - magic `ORPTRACE`, then `u32` version (1);
//! - `u32` frame-counter address, `u32` region count, then `u32` base and `u32` size per
//!   region;
//! - records until the end of the file: `u32` frame-counter value, `f64` seconds since
//!   capture start (host clock), then the regions' bytes back to back.
//!
//! Traces hold raw game memory, so they belong in `/cache` and are never committed.

use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use thiserror::Error;

use crate::layout::Layout;

const MAGIC: &[u8; 8] = b"ORPTRACE";
const VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum TraceError {
    #[error("trace I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("not a pine-probe trace, or an unsupported version")]
    BadHeader,
    #[error("trace regions do not match the layout (was it captured with another layout?)")]
    LayoutMismatch,
    #[error("trace ends in the middle of a record")]
    Truncated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceHeader {
    pub frame_counter: u32,
    /// `(base, size)` per region, in capture order.
    pub regions: Vec<(u32, u32)>,
}

impl TraceHeader {
    pub fn from_layout(layout: &Layout) -> Self {
        Self {
            frame_counter: layout.frame_counter,
            regions: layout.regions.iter().map(|r| (r.base, r.size)).collect(),
        }
    }

    pub fn sample_len(&self) -> usize {
        self.regions.iter().map(|&(_, size)| size as usize).sum()
    }

    pub fn check_layout(&self, layout: &Layout) -> Result<(), TraceError> {
        if *self == Self::from_layout(layout) {
            Ok(())
        } else {
            Err(TraceError::LayoutMismatch)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub frame: u32,
    pub time: f64,
    pub data: Vec<u8>,
}

pub struct TraceWriter<W: Write> {
    out: W,
}

impl TraceWriter<BufWriter<std::fs::File>> {
    pub fn create(path: impl AsRef<Path>, header: &TraceHeader) -> Result<Self, TraceError> {
        Self::new(BufWriter::new(std::fs::File::create(path)?), header)
    }
}

impl<W: Write> TraceWriter<W> {
    pub fn new(mut out: W, header: &TraceHeader) -> Result<Self, TraceError> {
        out.write_all(MAGIC)?;
        out.write_all(&VERSION.to_le_bytes())?;
        out.write_all(&header.frame_counter.to_le_bytes())?;
        out.write_all(&(header.regions.len() as u32).to_le_bytes())?;
        for &(base, size) in &header.regions {
            out.write_all(&base.to_le_bytes())?;
            out.write_all(&size.to_le_bytes())?;
        }
        Ok(Self { out })
    }

    pub fn write(&mut self, record: &Record) -> Result<(), TraceError> {
        self.out.write_all(&record.frame.to_le_bytes())?;
        self.out.write_all(&record.time.to_le_bytes())?;
        self.out.write_all(&record.data)?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, TraceError> {
        self.out.flush()?;
        Ok(self.out)
    }
}

pub struct TraceReader<R: Read> {
    input: R,
    pub header: TraceHeader,
}

impl TraceReader<BufReader<std::fs::File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TraceError> {
        Self::new(BufReader::new(std::fs::File::open(path)?))
    }
}

fn read_u32(r: &mut impl Read) -> Result<u32, TraceError> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(|_| TraceError::BadHeader)?;
    Ok(u32::from_le_bytes(b))
}

impl<R: Read> TraceReader<R> {
    pub fn new(mut input: R) -> Result<Self, TraceError> {
        let mut magic = [0u8; 8];
        input
            .read_exact(&mut magic)
            .map_err(|_| TraceError::BadHeader)?;
        if &magic != MAGIC || read_u32(&mut input)? != VERSION {
            return Err(TraceError::BadHeader);
        }
        let frame_counter = read_u32(&mut input)?;
        let count = read_u32(&mut input)?;
        if count > 64 {
            return Err(TraceError::BadHeader);
        }
        let mut regions = Vec::with_capacity(count as usize);
        for _ in 0..count {
            regions.push((read_u32(&mut input)?, read_u32(&mut input)?));
        }
        Ok(Self {
            input,
            header: TraceHeader {
                frame_counter,
                regions,
            },
        })
    }

    /// The next record, or `None` at a clean end of file.
    pub fn next_record(&mut self) -> Result<Option<Record>, TraceError> {
        let mut head = [0u8; 12];
        let mut got = 0;
        while got < head.len() {
            match self.input.read(&mut head[got..])? {
                0 if got == 0 => return Ok(None),
                0 => return Err(TraceError::Truncated),
                n => got += n,
            }
        }
        let mut data = vec![0u8; self.header.sample_len()];
        self.input
            .read_exact(&mut data)
            .map_err(|_| TraceError::Truncated)?;
        Ok(Some(Record {
            frame: u32::from_le_bytes(head[..4].try_into().unwrap()),
            time: f64::from_le_bytes(head[4..].try_into().unwrap()),
            data,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_records() {
        let header = TraceHeader {
            frame_counter: 0x1234,
            regions: vec![(0x100, 3), (0x200, 2)],
        };
        let records = [
            Record {
                frame: 7,
                time: 0.5,
                data: vec![1, 2, 3, 4, 5],
            },
            Record {
                frame: 8,
                time: 0.52,
                data: vec![6, 7, 8, 9, 10],
            },
        ];
        let mut writer = TraceWriter::new(Vec::new(), &header).unwrap();
        for r in &records {
            writer.write(r).unwrap();
        }
        let bytes = writer.finish().unwrap();

        let mut reader = TraceReader::new(bytes.as_slice()).unwrap();
        assert_eq!(reader.header, header);
        assert_eq!(reader.next_record().unwrap().as_ref(), Some(&records[0]));
        assert_eq!(reader.next_record().unwrap().as_ref(), Some(&records[1]));
        assert_eq!(reader.next_record().unwrap(), None);

        let cut = &bytes[..bytes.len() - 2];
        let mut reader = TraceReader::new(cut).unwrap();
        reader.next_record().unwrap();
        assert!(matches!(reader.next_record(), Err(TraceError::Truncated)));
        assert!(matches!(
            TraceReader::new(&b"ORPTRACX"[..]),
            Err(TraceError::BadHeader)
        ));
    }
}
