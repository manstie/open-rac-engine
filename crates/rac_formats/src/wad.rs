//! The games' "WAD" compression (PLAN.md P2.4): an LZ77 stream close to LZO1X, read in
//! 0x2000-byte blocks. Written from UYA NTSC-U's own decompressor (`wad_decompress`,
//! 0x433EA0; `docs/research/disc-layout.md`, "WAD compression").
//!
//! A WAD is a 16-byte header (`"WAD"`, then the whole WAD's size as an unaligned `u32`)
//! followed by packets. The game streams the packets through scratchpad memory one block
//! at a time, so a packet never crosses a block: each block ends with a packet that moves
//! to the next one, and the rest of the block is padding.

use crate::FormatError;

const HEADER: usize = 0x10;
const BLOCK: usize = 0x2000;
/// No WAD on our discs unpacks to more than this; a bigger result means bad data.
const MAX_OUTPUT: usize = 256 << 20;

fn bad(at: usize, detail: impl Into<String>) -> FormatError {
    FormatError::Invalid {
        what: "WAD packet",
        address: at as u32,
        detail: detail.into(),
    }
}

/// The size a WAD at the start of `bytes` says it has, header included, or `None` if
/// `bytes` doesn't start with a WAD.
pub fn wad_size(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 7 || &bytes[..3] != b"WAD" {
        return None;
    }
    Some(u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]) as usize)
}

/// Reads the WAD at the start of `src` and returns what it unpacks to.
pub fn decompress(src: &[u8]) -> Result<Vec<u8>, FormatError> {
    let total = wad_size(src).ok_or_else(|| bad(0, "no WAD header"))?;
    if total > src.len() {
        return Err(FormatError::OutOfBounds {
            address: 0,
            len: total,
        });
    }
    if total <= HEADER {
        return Ok(Vec::new());
    }
    let mut reader = Reader {
        src: &src[..total],
        pos: HEADER,
        block: 0,
        end: 0,
    };
    reader.end = reader.block_end();
    let mut out: Vec<u8> = Vec::new();

    // A first byte above 0x11 is a literal run of that many bytes, less 0x11.
    let first = reader.peek()? as usize;
    let mut after_literals = false;
    if first > 0x11 {
        reader.pos += 1;
        out.extend_from_slice(reader.take(first - 0x11)?);
        after_literals = true;
    }

    while reader.pos < reader.end {
        let at = reader.pos;
        let flag = reader.byte()?;
        if flag < 0x10 {
            if after_literals {
                // The game stops here (a trap): literal runs only follow a match.
                return Err(bad(at, format!("literal run {flag:#x} after literals")));
            }
            let count = if flag == 0 {
                reader.byte()? as usize + 0x12
            } else {
                flag as usize + 3
            };
            out.extend_from_slice(reader.take(count)?);
            after_literals = true;
            continue;
        }
        let (distance, length, low) = if flag >= 0x40 {
            let next = reader.byte()? as usize;
            let distance = 1 + ((flag as usize >> 2) & 7) + (next << 3);
            (distance, (flag as usize >> 5) + 1, flag)
        } else if flag >= 0x20 {
            let mut length = flag as usize & 0x1F;
            if length == 0 {
                length = reader.byte()? as usize + 0x1F;
            }
            let (low, high) = (reader.byte()?, reader.byte()?);
            let distance = 1 + (low as usize >> 2) + ((high as usize) << 6);
            (distance, length + 2, low)
        } else {
            let mut length = flag as usize & 7;
            if length == 0 {
                length = reader.byte()? as usize + 7;
            }
            let (low, high) = (reader.byte()?, reader.byte()?);
            let far = ((flag as usize & 8) << 11) + (low as usize >> 2) + ((high as usize) << 6);
            if far == 0 {
                if length != 1 {
                    // The end of this block: the rest is padding.
                    reader.next_block();
                    after_literals = false;
                    continue;
                }
                // A packet that copies nothing, though its trailing literals still
                // count; it lets one literal run follow another.
                let trailing = low as usize & 3;
                out.extend_from_slice(reader.take(trailing)?);
                after_literals = trailing > 0;
                continue;
            }
            (far + 0x4000, length + 2, low)
        };
        let start = out
            .len()
            .checked_sub(distance)
            .ok_or_else(|| bad(at, format!("looks back {distance} with {} out", out.len())))?;
        if out.len() + length > MAX_OUTPUT {
            return Err(bad(at, "unpacks to more than 256 MB"));
        }
        // Copies can overlap what they write, so go a byte at a time.
        for i in 0..length {
            out.push(out[start + i]);
        }
        let trailing = low as usize & 3;
        out.extend_from_slice(reader.take(trailing)?);
        after_literals = trailing > 0;
    }
    Ok(out)
}

struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
    block: usize,
    /// Where the data in this block ends (beyond it, for every block but the last).
    end: usize,
}

impl<'a> Reader<'a> {
    fn block_end(&self) -> usize {
        let start = HEADER + self.block * BLOCK;
        if self.src.len() - start < BLOCK {
            self.src.len()
        } else {
            usize::MAX
        }
    }

    fn next_block(&mut self) {
        self.block += 1;
        self.pos = HEADER + self.block * BLOCK;
        self.end = if self.pos >= self.src.len() {
            self.pos
        } else {
            self.block_end()
        };
    }

    fn peek(&self) -> Result<u8, FormatError> {
        self.src
            .get(self.pos)
            .copied()
            .ok_or(FormatError::OutOfBounds {
                address: self.pos as u32,
                len: 1,
            })
    }

    fn byte(&mut self) -> Result<u8, FormatError> {
        let value = self.peek()?;
        self.pos += 1;
        Ok(value)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], FormatError> {
        let bytes = self
            .src
            .get(self.pos..self.pos + count)
            .ok_or(FormatError::OutOfBounds {
                address: self.pos as u32,
                len: count,
            })?;
        self.pos += count;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAD around `packets`, already split into blocks by the caller.
    fn wad(packets: &[u8]) -> Vec<u8> {
        let mut out = b"WAD".to_vec();
        out.extend(((packets.len() + HEADER) as u32).to_le_bytes());
        out.resize(HEADER, 0);
        out.extend_from_slice(packets);
        out
    }

    /// Packs `data` as literal runs only, separated by empty packets, ending each block
    /// with a block switch. Good enough to make long histories for the tests.
    fn literals_only(data: &[u8]) -> Vec<u8> {
        let mut packets = Vec::new();
        let mut block_start = 0;
        let mut first = true;
        for chunk in data.chunks(200) {
            // Room for a separator, the run's header and bytes, and a block switch.
            if packets.len() - block_start + chunk.len() + 8 > BLOCK {
                packets.extend([0x12, 0, 0]);
                packets.resize(block_start + BLOCK, 0xEE);
                block_start = packets.len();
                first = true;
            }
            if !first {
                packets.extend([0x11, 0, 0]);
            }
            packets.extend([0, (chunk.len() - 0x12) as u8]);
            packets.extend_from_slice(chunk);
            first = false;
        }
        packets
    }

    #[test]
    fn a_first_byte_above_0x11_is_a_literal_run() {
        assert_eq!(decompress(&wad(&[0x14, b'a', b'b', b'c'])).unwrap(), b"abc");
    }

    #[test]
    fn short_and_medium_matches_copy_from_behind() {
        // "abcd", then a short match of 3 from 4 back with one trailing literal ('e'),
        // then a medium match of 5 from 2 back.
        let packets = [
            0x01,
            b'a',
            b'b',
            b'c',
            b'd', //
            0x40 | (3 << 2) | 1,
            0,
            b'e', //
            0x23,
            1 << 2,
            0,
        ];
        assert_eq!(
            decompress(&wad(&packets)).unwrap(),
            b"abcdabcececec".as_slice()
        );
    }

    #[test]
    fn long_lengths_take_an_extra_byte() {
        // One literal, then a medium match 1 back of 0x1F + 2 + 9 bytes.
        let packets = [0x15, b'z', b'y', b'x', b'w', 0x20, 9, 0, 0];
        let out = decompress(&wad(&packets)).unwrap();
        assert_eq!(out.len(), 4 + 0x1F + 2 + 9);
        assert!(out[4..].iter().all(|&b| b == b'w'));
    }

    #[test]
    fn blocks_end_with_a_switch_and_padding() {
        let mut packets = vec![0x13, b'x', b'y', 0x12, 0, 0];
        packets.resize(BLOCK, 0xEE);
        // The next block starts with an empty packet carrying one literal.
        packets.extend([0x11, 1, 0, b'z']);
        assert_eq!(decompress(&wad(&packets)).unwrap(), b"xyz");
    }

    #[test]
    fn far_matches_reach_past_16k() {
        let data: Vec<u8> = (0..0x4100u32).map(|i| (i * 7 + i / 255) as u8).collect();
        let mut packets = literals_only(&data);
        // Copy 4 bytes from 0x4100 back (the very start): far = 0x100.
        packets.extend([0x12, 0x100u16 as u8 & 0xFC, (0x100 >> 6) as u8]);
        let out = decompress(&wad(&packets)).unwrap();
        assert_eq!(&out[..data.len()], data.as_slice());
        assert_eq!(&out[data.len()..], &data[..4]);
    }

    #[test]
    fn bad_streams_are_errors_not_panics() {
        // Looking back before the start.
        assert!(decompress(&wad(&[0x14, 1, 2, 3, 0x40, 9])).is_err());
        // A literal run straight after literals.
        assert!(decompress(&wad(&[0x14, 1, 2, 3, 0x01, 4, 5, 6, 7])).is_err());
        // Running out of input mid-packet.
        assert!(decompress(&wad(&[0x14, 1, 2, 3, 0x20])).is_err());
        // A size bigger than the data.
        let mut short = wad(&[0x14, 1, 2, 3]);
        short[3] = 0xFF;
        assert!(decompress(&short).is_err());
        assert!(decompress(b"NOPE").is_err());
    }

    #[test]
    fn an_empty_wad_is_empty() {
        assert_eq!(decompress(&wad(&[])).unwrap(), b"");
    }
}
