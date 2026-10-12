//! Keeps PCSX2 from "ejecting" memory cards when a savestate is loaded.
//!
//! A savestate records a checksum for each memory card. On load, PCSX2 compares those with
//! the cards it has open. If any differ, it ejects every card for a moment so the game
//! notices the change, and R&C3 then pauses on a "memory card removed" message. That would
//! derail an input-recording replay whenever the card was written after the savestate was
//! made, which autosave does all the time.
//!
//! For an 8 MiB PS2 card file, PCSX2 keeps the checksum as a `u64` at byte 0x210 of the
//! file (`FileMemoryCard` in `pcsx2/SIO/Memcard/MemoryCardFile.cpp`, GPL, read for facts
//! only). The savestate stores the checksums as eight `u64`s in
//! `PCSX2 Internal Structures.dat`, ordered port 1 slots 1–4 then port 2 slots 1–4. Slots
//! 2–4 are multitap slots, which read as 0 when disabled. The block's position in that
//! file isn't fixed, so [`find_stored_checksums`] searches for it.
//!
//! The replay harness patches *copies* of the owner's cards, never the originals.

/// Size of a standard 8 MiB PS2 card image (with ECC).
pub const PS2_CARD_SIZE: usize = 0x84_0000;
/// Where PCSX2 keeps a PS2 card's running checksum.
pub const CHECKSUM_OFFSET: usize = 0x210;

/// The checksum PCSX2 will report for a PS2 card file, or `None` for other sizes.
pub fn card_checksum(card: &[u8]) -> Option<u64> {
    (card.len() == PS2_CARD_SIZE).then(|| {
        u64::from_le_bytes(
            card[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 8]
                .try_into()
                .unwrap(),
        )
    })
}

pub fn set_card_checksum(card: &mut [u8], value: u64) {
    card[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 8].copy_from_slice(&value.to_le_bytes());
}

/// Finds the stored `[port 1, port 2]` checksums in a savestate's internal structures.
///
/// With multitaps off the block looks like `[a, 0, 0, 0, b, 0, 0, 0]`. To avoid matching
/// stray zero runs, at least one of `a` and `b` must equal the current checksum of that
/// card, which holds whenever only one card was written since the savestate. Returns
/// `None` unless exactly one candidate is found.
pub fn find_stored_checksums(internal: &[u8], current: [u64; 2]) -> Option<[u64; 2]> {
    let word = |at: usize| u64::from_le_bytes(internal[at..at + 8].try_into().unwrap());
    let mut found = None;
    for at in 0..internal.len().saturating_sub(63) {
        if [1, 2, 3, 5, 6, 7].iter().any(|&i| word(at + 8 * i) != 0) {
            continue;
        }
        let (a, b) = (word(at), word(at + 32));
        if a == 0 || b == 0 || (a != current[0] && b != current[1]) {
            continue;
        }
        if found.is_some_and(|f| f != [a, b]) {
            return None;
        }
        found = Some([a, b]);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_block_next_to_an_unchanged_card() {
        let mut internal = vec![0x55u8; 4000];
        internal[1000..1064].fill(0);
        internal[1000..1008].copy_from_slice(&0xDB33_6DFC_E2E8_198Eu64.to_le_bytes());
        internal[1032..1040].copy_from_slice(&u64::MAX.to_le_bytes());
        // Card 1 changed since the savestate; card 2 did not.
        let found = find_stored_checksums(&internal, [0x4E40_04D4_5888_6624, u64::MAX]);
        assert_eq!(found, Some([0xDB33_6DFC_E2E8_198E, u64::MAX]));
        // Neither card matches: refuse to guess.
        assert_eq!(find_stored_checksums(&internal, [1, 2]), None);
    }

    #[test]
    fn reads_and_writes_the_card_checksum() {
        let mut card = vec![0u8; PS2_CARD_SIZE];
        set_card_checksum(&mut card, 0x0123_4567_89AB_CDEF);
        assert_eq!(card_checksum(&card), Some(0x0123_4567_89AB_CDEF));
        assert_eq!(card_checksum(&card[..100]), None);
    }
}
