#!/usr/bin/env python3
# Finds a function of one level build in another build's EE memory (PLAN.md RE.10).
# Deadlocked links almost the whole game into every level, at different addresses, with
# its own .data and .bss, so globals move between levels too (docs/research/coop.md,
# "Status and method"). This carries an address read in one build over to another.
#
# usage: python3 -I tools/ghidra/match-function.py <from.ee.bin> <to.ee.bin> <gp hex>
#            <address hex> [words]
#
# The images are cache/ghidra/ee/<name>.ee.bin (import-savestate.sh); gp comes from
# `pine-probe savestate-regs` and must be the same in both. The first `words` instructions
# (default 24) from the address are compared with the parts that change between builds
# masked out: jump targets and the immediates of lui, addiu, ori, slti and loads and
# stores. With one match, each instruction whose operand differs is printed with both
# builds' values: a jal's target, a gp-relative global's address, or the raw immediate
# (an absolute global is lui's high half plus a load's or store's low half).
import struct
import sys

# Opcodes whose 16-bit immediate is an address or constant that moves between builds.
IMMEDIATE_OPS = {
    0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0F,  # addi(u), slti(u), andi, ori, lui
    0x1E, 0x1F,  # lq, sq
    0x20, 0x21, 0x23, 0x24, 0x25, 0x28, 0x29, 0x2B,  # loads and stores
    0x31, 0x37, 0x39, 0x3F,  # lwc1, ld, swc1, sd
}


def word(data, at):
    return struct.unpack_from('<I', data, at)[0]


def masked(w):
    op = w >> 26
    if op in (2, 3):  # j, jal
        return w & 0xFC000000
    if op in IMMEDIATE_OPS:
        return w & 0xFFFF0000
    return w


def signed16(w):
    return struct.unpack('<h', struct.pack('<H', w & 0xFFFF))[0]


def main():
    if len(sys.argv) < 5:
        sys.exit(__doc__ or 'usage: match-function.py <from> <to> <gp hex> <address hex> [words]')
    src = open(sys.argv[1], 'rb').read()
    dst = open(sys.argv[2], 'rb').read()
    gp = int(sys.argv[3], 16)
    addr = int(sys.argv[4], 16)
    n = int(sys.argv[5]) if len(sys.argv) > 5 else 24

    want = [masked(word(src, addr + 4 * i)) for i in range(n)]
    hits = [
        a for a in range(0x100000, len(dst) - 4 * n, 4)
        if masked(word(dst, a)) == want[0]
        and all(masked(word(dst, a + 4 * i)) == want[i] for i in range(1, n))
    ]
    print('matches:', ' '.join(hex(h) for h in hits) or 'none')
    if len(hits) != 1:
        return
    for i in range(n):
        w0 = word(src, addr + 4 * i)
        w1 = word(dst, hits[0] + 4 * i)
        if w0 == w1:
            continue
        if w0 >> 26 in (2, 3):
            note = 'jal %x -> %x' % ((w0 & 0x3FFFFFF) << 2, (w1 & 0x3FFFFFF) << 2)
        elif (w0 >> 21) & 31 == 28:
            note = 'gp %x -> %x' % (gp + signed16(w0), gp + signed16(w1))
        else:
            note = 'imm %#06x -> %#06x' % (w0 & 0xFFFF, w1 & 0xFFFF)
        print('%x / %x: %s' % (addr + 4 * i, hits[0] + 4 * i, note))


if __name__ == '__main__':
    main()
