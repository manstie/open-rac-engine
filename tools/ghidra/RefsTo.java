// Lists every reference to the given addresses: the instruction, whether it reads or writes,
// and the function it is in. Run it as a -postScript on an analysed import, with $gp set,
// so gp-relative loads and stores resolve (import-savestate.sh does that).
//
// Use it to find who writes a field seen in a trace, for example the hero's wall-contact
// flag. The output names game addresses only; still keep it in cache/.
//
// args: <out file> <address hex> ...
//@category open-rac-engine

import java.io.FileWriter;
import java.io.PrintWriter;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;

public class RefsTo extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("usage: RefsTo.java <out file> <address hex> ...");
            return;
        }
        AddressSpace space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        try (PrintWriter out = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                Address target = space.getAddress(Long.parseLong(args[i], 16));
                out.println("== " + target);
                for (Reference ref : getReferencesTo(target)) {
                    Address from = ref.getFromAddress();
                    Function f = getFunctionContaining(from);
                    Instruction insn = getInstructionAt(from);
                    out.println("  " + from + " " + ref.getReferenceType()
                        + " in " + (f == null ? "?" : f.getName() + "@" + f.getEntryPoint())
                        + (insn == null ? "" : "  " + insn));
                }
            }
        }
    }
}
