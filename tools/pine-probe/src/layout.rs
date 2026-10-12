//! Per-release memory layouts, loaded from `re/<serial>/hero.toml`.
//!
//! A layout names the memory regions to capture each frame, the address of a counter that
//! advances once per frame, and typed fields inside the regions. See `re/README.md` for
//! the file format and for how each value was verified.

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("cannot read layout: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid layout TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid layout: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    Vec3,
    Vec4,
}

impl FieldType {
    pub fn size(self) -> u32 {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::Vec3 => 12,
            Self::Vec4 => 16,
        }
    }

    /// Number of CSV columns the field expands to.
    pub fn columns(self) -> u32 {
        match self {
            Self::Vec3 => 3,
            Self::Vec4 => 4,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub name: String,
    pub base: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub region: String,
    pub offset: u32,
    #[serde(rename = "type")]
    pub ty: FieldType,
    /// Fields marked `key` decide which sample wins when a frame is read several times
    /// (see [`crate::capture`]). Pick fields the game writes once per frame.
    #[serde(default)]
    pub key: bool,
    /// How the field was identified and verified. Kept with the data on purpose.
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// Disc serial, for example `SCES-52456`.
    pub release: String,
    pub title: String,
    /// Display refresh rate the game runs at: 50 for PAL, 60 for NTSC.
    pub refresh_hz: u32,
    /// Address of a `u32` that the game increments exactly once per frame.
    pub frame_counter: u32,
    #[serde(default)]
    pub notes: String,
    #[serde(rename = "region")]
    pub regions: Vec<Region>,
    #[serde(rename = "field", default)]
    pub fields: Vec<Field>,
}

/// Where a field's bytes sit inside a capture sample (all regions back to back).
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub start: usize,
    pub len: usize,
}

impl Layout {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LayoutError> {
        Self::from_toml(&std::fs::read_to_string(path)?)
    }

    pub fn from_toml(text: &str) -> Result<Self, LayoutError> {
        let layout: Self = toml::from_str(text)?;
        layout.validate()?;
        Ok(layout)
    }

    fn validate(&self) -> Result<(), LayoutError> {
        let bad = |msg: String| Err(LayoutError::Invalid(msg));
        if self.regions.is_empty() {
            return bad("at least one [[region]] is required".into());
        }
        for (i, r) in self.regions.iter().enumerate() {
            if self.regions[..i].iter().any(|o| o.name == r.name) {
                return bad(format!("duplicate region {:?}", r.name));
            }
            if r.size == 0 || r.base.checked_add(r.size).is_none() {
                return bad(format!("region {:?} has an invalid extent", r.name));
            }
        }
        for (i, f) in self.fields.iter().enumerate() {
            if self.fields[..i].iter().any(|o| o.name == f.name) {
                return bad(format!("duplicate field {:?}", f.name));
            }
            let Some(region) = self.regions.iter().find(|r| r.name == f.region) else {
                return bad(format!(
                    "field {:?} names unknown region {:?}",
                    f.name, f.region
                ));
            };
            if u64::from(f.offset) + u64::from(f.ty.size()) > u64::from(region.size) {
                return bad(format!(
                    "field {:?} (+{:#x}) runs past region {:?}",
                    f.name, f.offset, f.region
                ));
            }
        }
        Ok(())
    }

    /// Total bytes in one capture sample.
    pub fn sample_len(&self) -> usize {
        self.regions.iter().map(|r| r.size as usize).sum()
    }

    /// Where `field` sits inside a capture sample.
    pub fn placement(&self, field: &Field) -> Placement {
        let mut start = 0usize;
        for r in &self.regions {
            if r.name == field.region {
                return Placement {
                    start: start + field.offset as usize,
                    len: field.ty.size() as usize,
                };
            }
            start += r.size as usize;
        }
        unreachable!("validated layouts only reference known regions")
    }

    /// Sample byte ranges that make up the voting key (see [`Field::key`]).
    pub fn key_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.fields
            .iter()
            .filter(|f| f.key)
            .map(|f| {
                let p = self.placement(f);
                p.start..p.start + p.len
            })
            .collect()
    }

    /// CSV header names for all fields, vectors expanded to `.x/.y/.z/.w`.
    pub fn columns(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.fields {
            match f.ty {
                FieldType::Vec3 | FieldType::Vec4 => {
                    for axis in ["x", "y", "z", "w"].iter().take(f.ty.columns() as usize) {
                        out.push(format!("{}.{axis}", f.name));
                    }
                }
                _ => out.push(f.name.clone()),
            }
        }
        out
    }

    /// Formats every field of one sample as CSV cells, in [`Self::columns`] order.
    pub fn format_sample(&self, sample: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.fields {
            let p = self.placement(f);
            let b = &sample[p.start..p.start + p.len];
            let word = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
            match f.ty {
                FieldType::U8 => out.push(b[0].to_string()),
                FieldType::I8 => out.push((b[0] as i8).to_string()),
                FieldType::U16 => out.push(u16::from_le_bytes([b[0], b[1]]).to_string()),
                FieldType::I16 => out.push(i16::from_le_bytes([b[0], b[1]]).to_string()),
                FieldType::U32 => out.push(word(0).to_string()),
                FieldType::I32 => out.push((word(0) as i32).to_string()),
                FieldType::F32 | FieldType::Vec3 | FieldType::Vec4 => {
                    for i in 0..f.ty.columns() as usize {
                        out.push(f32::from_bits(word(4 * i)).to_string());
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        release = "SCES-00000"
        title = "Test"
        refresh_hz = 50
        frame_counter = 0x1000

        [[region]]
        name = "a"
        base = 0x2000
        size = 0x10

        [[region]]
        name = "hero"
        base = 0x3000
        size = 0x40

        [[field]]
        name = "pos"
        region = "hero"
        offset = 0x10
        type = "vec3"
        key = true

        [[field]]
        name = "state"
        region = "hero"
        offset = 0x20
        type = "i32"
    "#;

    #[test]
    fn parses_places_and_formats_fields() {
        let layout = Layout::from_toml(SAMPLE).unwrap();
        assert_eq!(layout.sample_len(), 0x50);
        assert_eq!(layout.key_ranges(), vec![0x20..0x2C]);
        assert_eq!(
            layout.columns(),
            ["pos.x", "pos.y", "pos.z", "state"].map(String::from)
        );
        let mut sample = vec![0u8; 0x50];
        sample[0x20..0x24].copy_from_slice(&1.5f32.to_le_bytes());
        sample[0x30..0x34].copy_from_slice(&(-7i32).to_le_bytes());
        assert_eq!(layout.format_sample(&sample), ["1.5", "0", "0", "-7"]);
    }

    #[test]
    fn rejects_inconsistent_layouts() {
        let past_end = SAMPLE.replace("offset = 0x20", "offset = 0x3E");
        assert!(matches!(
            Layout::from_toml(&past_end),
            Err(LayoutError::Invalid(_))
        ));
        let unknown = SAMPLE.replace(
            "region = \"hero\"\n        offset = 0x20",
            "region = \"x\"\n        offset = 0x20",
        );
        assert!(matches!(
            Layout::from_toml(&unknown),
            Err(LayoutError::Invalid(_))
        ));
    }

    #[test]
    fn committed_layouts_are_valid() {
        let re = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../re");
        let mut seen = 0;
        for entry in std::fs::read_dir(&re).unwrap() {
            let path = entry.unwrap().path().join("hero.toml");
            if path.exists() {
                let layout = Layout::load(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                assert!(
                    path.parent().unwrap().ends_with(&layout.release),
                    "{path:?} is in the wrong folder"
                );
                assert!(!layout.key_ranges().is_empty(), "{path:?} needs key fields");
                seen += 1;
            }
        }
        assert!(seen > 0, "no layouts found under re/");
    }
}
