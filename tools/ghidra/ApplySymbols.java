// Names functions and data in a Ghidra import from re/<serial>/symbols.toml, and puts each
// symbol's note in a plate comment. Run it as a -postScript (without -readOnly) on an
// import of the memory image the file was written for (its `image` key).
//
// Only the [[symbol]] tables are read, one `key = value` per line, which is the format
// re/README.md describes. `cargo test -p pine-probe` checks the files themselves.
//
// args: <symbols.toml>
//@category open-rac-engine

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

public class ApplySymbols extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) {
            printerr("usage: ApplySymbols.java <symbols.toml>");
            return;
        }
        List<String> lines = Files.readAllLines(Path.of(args[0]));
        Map<String, String> current = null;
        int applied = 0;
        for (String raw : lines) {
            String line = raw.trim();
            if (line.equals("[[symbol]]")) {
                if (current != null) {
                    applied += apply(current);
                }
                current = new HashMap<>();
                continue;
            }
            int eq = line.indexOf('=');
            if (current == null || eq < 0 || line.startsWith("#")) {
                continue;
            }
            String value = line.substring(eq + 1).trim();
            if (value.startsWith("\"") && value.endsWith("\"") && value.length() >= 2) {
                value = value.substring(1, value.length() - 1).replace("\\\"", "\"");
            }
            current.put(line.substring(0, eq).trim(), value);
        }
        if (current != null) {
            applied += apply(current);
        }
        println("applied " + applied + " symbols from " + args[0]);
    }

    private int apply(Map<String, String> s) throws Exception {
        String name = s.get("name");
        String address = s.get("address");
        if (name == null || address == null) {
            printerr("skipping a symbol without name or address: " + s);
            return 0;
        }
        Address at = toAddr(Long.decode(address.replace("_", "")));
        if ("function".equals(s.get("kind"))) {
            Function f = getFunctionAt(at);
            if (f == null) {
                if (getInstructionAt(at) == null) {
                    disassemble(at);
                }
                f = createFunction(at, name);
            }
            if (f == null) {
                printerr("could not create function " + name + " at " + at);
                return 0;
            }
            f.setName(name, SourceType.USER_DEFINED);
        } else {
            createLabel(at, name, true, SourceType.USER_DEFINED);
        }
        String note = s.getOrDefault("note", "");
        String confidence = s.getOrDefault("confidence", "?");
        setPlateComment(at, name + " (" + confidence + "): " + note);
        return 1;
    }
}
