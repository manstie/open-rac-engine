// Prepares a raw EE-memory import (PLAN.md RE.1, RE.3): sets $gp, finds the code regions
// and creates functions at prologues and call targets, so auto-analysis has somewhere to
// start. Run it as a -preScript. See docs/re-setup.md, "Static analysis".
//
// args: <gp hex> [<start hex> <end hex> ...]
//   gp: the game's $gp, from `pine-probe savestate-regs`. Without it, gp-relative globals
//       resolve to nonsense.
//   ranges: code regions to seed. Without them, regions are detected: 16 KiB blocks at or
//       above 0x100000 with at least three `jr ra` and three stack-frame prologues, with
//       one-block gaps merged.
//@category open-rac-engine

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.lang.Register;

public class SeedFunctions extends GhidraScript {
    private static final int JR_RA = 0x03E00008;
    private static final int BLOCK = 0x4000;

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("usage: SeedFunctions.java <gp hex> [<start hex> <end hex> ...]");
            return;
        }
        AddressSpace space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        // The size of the RAM block at 0. The PS2 extension may map a BIOS ROM block far
        // above it, so the program's highest address is not the end of RAM.
        long size = currentProgram.getMemory().getBlock(space.getAddress(0)).getEnd().getOffset() + 1;

        long gp = Long.parseLong(args[0], 16);
        Register gpReg = currentProgram.getRegister("gp");
        currentProgram.getProgramContext().setValue(gpReg, space.getAddress(0),
            space.getAddress(size - 1), BigInteger.valueOf(gp));

        List<long[]> ranges = new ArrayList<>();
        for (int i = 1; i + 1 < args.length; i += 2) {
            ranges.add(new long[] { Long.parseLong(args[i], 16), Long.parseLong(args[i + 1], 16) });
        }
        if (ranges.isEmpty()) {
            ranges = detectCode(space, size);
        }
        for (long[] r : ranges) {
            println(String.format("code region %08x-%08x", r[0], r[1]));
        }

        // A prologue right after "jr ra; <delay slot>" counts as two votes, so it is seeded
        // even when nothing calls it directly (virtual functions, jump tables).
        TreeMap<Long, Integer> votes = new TreeMap<>();
        for (long[] r : ranges) {
            int[] words = readWords(space, r[0], r[1] - r[0]);
            for (int i = 2; i < words.length; i++) {
                int w = words[i];
                boolean prologue = (w >>> 16) == 0x27BD && (w & 0x8000) != 0;
                if (prologue && words[i - 2] == JR_RA) {
                    votes.merge(r[0] + 4L * i, 2, Integer::sum);
                }
                if ((w >>> 26) == 3) {
                    long target = (long) (w & 0x03FFFFFF) << 2;
                    if (inside(ranges, target)) {
                        votes.merge(target, 1, Integer::sum);
                    }
                }
            }
        }
        int made = 0;
        for (Map.Entry<Long, Integer> e : votes.entrySet()) {
            if (e.getValue() < 2) {
                continue;
            }
            Address at = space.getAddress(e.getKey());
            if (getFunctionAt(at) != null) {
                continue;
            }
            if (getInstructionAt(at) == null) {
                disassemble(at);
            }
            if (createFunction(at, null) != null) {
                made++;
            }
        }
        println(String.format("gp=%08x, %d candidates, %d functions created", gp, votes.size(), made));
    }

    private int[] readWords(AddressSpace space, long start, long length) throws Exception {
        byte[] buf = new byte[(int) length];
        currentProgram.getMemory().getBytes(space.getAddress(start), buf);
        int[] words = new int[buf.length / 4];
        for (int i = 0; i < words.length; i++) {
            words[i] = (buf[4 * i] & 0xff) | (buf[4 * i + 1] & 0xff) << 8
                | (buf[4 * i + 2] & 0xff) << 16 | (buf[4 * i + 3] & 0xff) << 24;
        }
        return words;
    }

    // Reads one block at a time: the headless analyzer's default heap is too small for
    // copies of all 32 MiB.
    private List<long[]> detectCode(AddressSpace space, long size) throws Exception {
        List<long[]> found = new ArrayList<>();
        for (long b = 0x100000 / BLOCK; b < size / BLOCK; b++) {
            int[] words = readWords(space, b * BLOCK, BLOCK);
            int returns = 0;
            int prologues = 0;
            for (int i = 0; i < words.length; i++) {
                if (words[i] == JR_RA) {
                    returns++;
                }
                if ((words[i] >>> 16) == 0x27BD) {
                    prologues++;
                }
            }
            if (returns < 3 || prologues < 3) {
                continue;
            }
            long start = b * BLOCK;
            long[] last = found.isEmpty() ? null : found.get(found.size() - 1);
            if (last != null && start - last[1] <= BLOCK) {
                last[1] = start + BLOCK;
            } else {
                found.add(new long[] { start, start + BLOCK });
            }
        }
        return found;
    }

    private static boolean inside(List<long[]> ranges, long at) {
        for (long[] r : ranges) {
            if (at >= r[0] && at < r[1]) {
                return true;
            }
        }
        return false;
    }
}
