// Writes the disassembly of address ranges to one file, for the places where the decompiler
// loses track of a value (VU0 macro code, registers live across a jump table). Run it as a
// -postScript on an analysed import.
//
// The output is disassembled game code: write it under cache/ (gitignored), never into the
// repository (CLAUDE.md, "Legal rules").
//
// args: <out file> <start hex> <end hex> [<start hex> <end hex> ...]
//@category open-rac-engine

import java.io.FileWriter;
import java.io.PrintWriter;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;

public class ListingAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 3 || args.length % 2 == 0) {
            printerr("usage: ListingAt.java <out file> <start hex> <end hex> ...");
            return;
        }
        AddressSpace space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        try (PrintWriter out = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i += 2) {
                Address start = space.getAddress(Long.parseLong(args[i], 16));
                Address end = space.getAddress(Long.parseLong(args[i + 1], 16));
                Function f = getFunctionContaining(start);
                out.println("== " + start + " .. " + end
                        + (f != null ? " in " + f.getName() : ""));
                Instruction ins = getInstructionAt(start);
                if (ins == null) {
                    ins = getInstructionAfter(start);
                }
                while (ins != null && ins.getAddress().compareTo(end) <= 0) {
                    out.println(ins.getAddress() + "  " + ins);
                    ins = ins.getNext();
                }
                out.println();
            }
        }
    }
}
