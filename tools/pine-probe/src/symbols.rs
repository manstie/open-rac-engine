//! Named code and data addresses per release, loaded from `re/<serial>/symbols.toml`.
//!
//! The names are ours. Each symbol says how sure we are and how it was identified, so
//! readers can tell a verified fact from a reading of the code. `tools/ghidra/ApplySymbols.java`
//! applies a file to a Ghidra import. See `re/README.md` for the format.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SymbolsError {
    #[error("cannot read symbols: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid symbols TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid symbols: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Function,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// A name inferred from context that nothing has confirmed yet.
    Low,
    /// Read from the code, but its behaviour has not been checked against the game.
    Medium,
    /// Behaviour checked against the running game (a trace, or a cross-check).
    High,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    pub name: String,
    pub address: u32,
    pub kind: Kind,
    pub confidence: Confidence,
    /// What the symbol does and how it was identified.
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbols {
    pub release: String,
    /// The memory image the addresses come from. Level code is a per-level overlay, so
    /// code addresses only hold for that level.
    pub image: String,
    #[serde(default)]
    pub notes: String,
    #[serde(rename = "symbol")]
    pub symbols: Vec<Symbol>,
}

impl Symbols {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SymbolsError> {
        Self::from_toml(&std::fs::read_to_string(path)?)
    }

    pub fn from_toml(text: &str) -> Result<Self, SymbolsError> {
        let symbols: Symbols = toml::from_str(text)?;
        symbols.validate()?;
        Ok(symbols)
    }

    fn validate(&self) -> Result<(), SymbolsError> {
        let invalid = |m: String| Err(SymbolsError::Invalid(m));
        let mut names = BTreeSet::new();
        let mut addresses = BTreeSet::new();
        for s in &self.symbols {
            let ident = !s.name.is_empty()
                && s.name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ident {
                return invalid(format!("{:?} is not a valid identifier", s.name));
            }
            if !names.insert(&s.name) {
                return invalid(format!("{} is defined twice", s.name));
            }
            if !addresses.insert(s.address) {
                return invalid(format!("{:#x} is named twice", s.address));
            }
            if s.address >= 0x0200_0000 {
                return invalid(format!("{}: {:#x} is outside EE memory", s.name, s.address));
            }
            if s.kind == Kind::Function && s.address % 4 != 0 {
                return invalid(format!("{}: functions are word-aligned", s.name));
            }
            if s.note.trim().is_empty() {
                return invalid(format!("{}: say how it was identified", s.name));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        release = "TEST-00000"
        image = "a level"

        [[symbol]]
        name = "hero_update"
        address = 0x004EE4E0
        kind = "function"
        confidence = "high"
        note = "calls the state update once per frame"

        [[symbol]]
        name = "hero"
        address = 0x001A4BE0
        kind = "data"
        confidence = "medium"
        note = "base used by the hero code"
    "#;

    #[test]
    fn parses_symbols() {
        let s = Symbols::from_toml(SAMPLE).unwrap();
        assert_eq!(s.symbols.len(), 2);
        assert_eq!(s.symbols[0].address, 0x004E_E4E0);
        assert_eq!(s.symbols[1].kind, Kind::Data);
        assert!(s.symbols[0].confidence > s.symbols[1].confidence);
    }

    #[test]
    fn rejects_inconsistent_symbols() {
        let dup = SAMPLE.replace("name = \"hero\"", "name = \"hero_update\"");
        assert!(matches!(
            Symbols::from_toml(&dup),
            Err(SymbolsError::Invalid(_))
        ));
        let misaligned = SAMPLE.replace("0x004EE4E0", "0x004EE4E2");
        assert!(matches!(
            Symbols::from_toml(&misaligned),
            Err(SymbolsError::Invalid(_))
        ));
        let unexplained = SAMPLE.replace("base used by the hero code", " ");
        assert!(matches!(
            Symbols::from_toml(&unexplained),
            Err(SymbolsError::Invalid(_))
        ));
        let bad_name = SAMPLE.replace("name = \"hero\"", "name = \"hero base\"");
        assert!(matches!(
            Symbols::from_toml(&bad_name),
            Err(SymbolsError::Invalid(_))
        ));
    }

    #[test]
    fn committed_symbols_are_valid() {
        let re = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../re");
        let mut seen = 0;
        for entry in std::fs::read_dir(&re).unwrap() {
            let path = entry.unwrap().path().join("symbols.toml");
            if path.exists() {
                let s = Symbols::load(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                assert!(
                    path.parent().unwrap().ends_with(&s.release),
                    "{path:?} is in the wrong folder"
                );
                seen += 1;
            }
        }
        assert!(seen > 0, "no symbols.toml found under re/");
    }
}
