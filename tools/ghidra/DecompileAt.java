// Decompiles the functions that contain the given addresses and writes them to one file,
// with each function's callers. Run it as a -postScript on an analysed import. An address
// no function contains gets a new function starting there (not saved with -readOnly).
//
// The output is decompiled game code: write it under cache/ (gitignored), never into the
// repository (CLAUDE.md, "Legal rules").
//
// args: <out file> <address hex> ...
//@category open-rac-engine

import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.HashSet;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.listing.Function;

public class DecompileAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("usage: DecompileAt.java <out file> <address hex> ...");
            return;
        }
        AddressSpace space = currentProgram.getAddressFactory().getDefaultAddressSpace();
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        Set<Function> done = new HashSet<>();
        try (PrintWriter out = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                Address at = space.getAddress(Long.parseLong(args[i], 16));
                Function f = getFunctionContaining(at);
                if (f == null) {
                    // Functions reached only through pointer tables (camera modes, for
                    // example) aren't found by analysis; make one where asked.
                    disassemble(at);
                    f = createFunction(at, null);
                }
                if (f == null) {
                    out.println("== no function contains " + at);
                    continue;
                }
                if (!done.add(f)) {
                    continue;
                }
                out.println("== " + f.getName() + " @ " + f.getEntryPoint() + " .. "
                    + f.getBody().getMaxAddress() + " (asked " + at + ")");
                out.print("callers:");
                for (Function caller : f.getCallingFunctions(monitor)) {
                    out.print(" " + caller.getEntryPoint());
                }
                out.println();
                DecompileResults r = decompiler.decompileFunction(f, 120, monitor);
                out.println(r.decompileCompleted() ? r.getDecompiledFunction().getC()
                        : "decompile failed: " + r.getErrorMessage());
            }
        }
        decompiler.dispose();
    }
}
