//! A level's gameplay file, as far as the hero and his camera need it so far: the rails
//! he grinds, the level's cameras, the camera volumes that tune the follow camera, the
//! shapes they test the hero against, and the level's paths (PLAN.md P3.10;
//! `docs/research/gameplay-format.md`).
//!
//! Read on UYA NTSC-U, from the disc and the game's level loader (`level_load_gameplay`,
//! 0x4252F0 in Veldin's build), which copies each block of the file into a list.

use crate::level::{LevelHeader, SECTOR, read_wad};
use crate::{DiscReader, FormatError, Image};

/// Where the gameplay file's header keeps the offset of each block we read.
const CAMERAS: u32 = 0x08;
const MOBYS: u32 = 0x4C;
const PVAR_TABLE: u32 = 0x5C;
const PVAR_DATA: u32 = 0x60;
const CUBOIDS: u32 = 0x68;
const SPHERES: u32 = 0x6C;
const CYLINDERS: u32 = 0x70;
const PATHS: u32 = 0x78;
const GRIND_PATHS: u32 = 0x7C;
const CAMERA_COLLISION: u32 = 0x88;

/// Bytes per grind path record: the bounding sphere, then four words.
const PATH_RECORD: u32 = 0x20;
/// Bytes per spline point: x, y, z, then a fourth float.
const POINT: u32 = 0x10;
/// Bytes per camera record: class, position, rotation, pvar index.
const CAMERA_RECORD: u32 = 0x20;
/// Bytes per cuboid, sphere or cylinder: its matrix, its inverse, its rotation.
const SHAPE_RECORD: u32 = 0x80;
/// The smallest moby record: the loader steps by each record's own size word.
const MOBY_RECORD: u32 = 0x88;
/// The camera collision grid: 64 × 64 cells of 16 u, one offset word each.
const GRID_CELLS: u32 = 64;
/// Bytes per camera collision shape in a grid cell's list.
const GRID_SHAPE: u32 = 0x30;

/// The moby class whose instances turn the hero's follow camera's settings while he is in
/// their cuboid (UYA's class 2329; `camera_volume_update`, 0x3093A0 in Veldin's build).
pub const CAMERA_VOLUME_CLASS: u32 = 2329;
/// The camera class of the path camera, which flies along one of the level's paths.
pub const PATH_CAMERA_CLASS: i32 = 0x13;
/// The camera class of the fixed-point camera, which puts the follow camera at the
/// instance's position while the hero is in its shape.
pub const FIXED_CAMERA_CLASS: i32 = 0x17;

/// A level's gameplay file, unpacked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gameplay {
    pub bytes: Vec<u8>,
}

/// A rail the hero can grind. He always moves along it in the order of its points.
#[derive(Debug, Clone, PartialEq)]
pub struct GrindPath {
    /// The centre of a sphere holding the whole rail; the game only looks for the
    /// nearest point of rails whose sphere holds the hero.
    pub centre: [f32; 3],
    pub radius: f32,
    /// The record's word that the game passes to its spline search with the spline.
    /// It is 0 on every rail on the UYA disc.
    pub flags: u32,
    /// The spline's points, in world units.
    pub points: Vec<[f32; 3]>,
}

/// One of the level's cameras. Its class is a camera mode: an entry in the game's camera
/// mode table (0x2F7700 in Veldin's build), whose functions decide when it takes over and
/// how it moves. Mode 0 is the follow camera.
#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    pub class: i32,
    pub position: [f32; 3],
    /// Euler angles in radians.
    pub rotation: [f32; 3],
    /// The camera's pvars, empty if it has none. [`Camera::trigger`] and
    /// [`Camera::path_camera`] read them.
    pub pvars: Vec<u8>,
}

/// The pvar fields every camera starts with: what makes it take over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTrigger {
    /// +0x00. Unread; 8.0 on Veldin's fixed-point camera, 0 elsewhere there.
    pub lead: f32,
    /// +0x08, +0x0C, +0x10: the sphere, cuboid and cylinder (indices into the level's
    /// shape lists) the hero must be in, for the classes that test a shape.
    pub sphere: Option<u32>,
    pub cuboid: Option<u32>,
    pub cylinder: Option<u32>,
    /// +0x14: a path the path camera tests the hero against when it has no shape.
    pub path: Option<u32>,
    /// +0x18: 1.5 on every camera seen. Unread.
    pub focus_height: f32,
    /// +0x1C: a camera with priority 0 never takes over; otherwise a higher priority
    /// takes over from a lower one.
    pub priority: u8,
    /// +0x1D: how the switch to this camera blends (`camera_switch`, 0x3F1670): 0, 2
    /// and 4 cut; 1 springs; 5 eases over a number of frames; 3 and 6 start from where
    /// the old camera was.
    pub blend_in: u8,
    /// +0x1E. Unread.
    pub blend_out: u8,
    /// +0x1F: when the camera may take over (`camera_may_take_over`, 0x3F1988): 0
    /// whenever it outranks the current one; 1 and 2 only when asked for; 4 while the
    /// hero is in its cuboid; 7 when the hero's state asks for its class; 3, 5 and 6
    /// only through its class's own test.
    pub activation: u8,
}

/// A path camera's own pvars (class 0x13): it flies along one path, looking at another
/// path or at a moby, then hands back to the follow camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathCamera {
    /// +0x20: the path the camera moves along.
    pub path: Option<u32>,
    /// +0x24: the path it looks at, at the same fraction of its length.
    pub look_path: Option<u32>,
    /// +0x28: a moby (by index) it looks at instead, plus `look_offset` (+0x2C).
    pub look_moby: Option<u32>,
    pub look_offset: [f32; 3],
    /// +0x3A: if positive, the flight's length in frames, which sets the speed.
    pub frames: i16,
    /// +0x3C: the speed along the path in u/s, when `frames` doesn't set it.
    pub speed: f32,
    /// +0x4C: probably the path camera to start when this one ends.
    pub next_camera: Option<u32>,
    /// +0xA0: a progress flag; once it is set the camera never takes over again.
    pub done_flag: Option<u32>,
}

/// A cuboid, sphere or cylinder from the level's shape lists. Cameras and camera volumes
/// test the hero against them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// The shape's axes (rows 0-2, each scaled to its half size) and its centre (row 3).
    pub matrix: [[f32; 4]; 4],
    /// The inverse of the axes: a point minus the centre, times these rows, gives the
    /// shape's own coordinates, -1 to 1 inside.
    pub inverse: [[f32; 4]; 3],
    /// Euler angles in radians.
    pub rotation: [f32; 3],
}

/// What kind of shape a camera collision entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
    Pill,
    Other(u32),
}

/// A moby placed in the level, as far as we read it.
#[derive(Debug, Clone, PartialEq)]
pub struct MobyInstance {
    pub class: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    /// The moby group it belongs to, or -1.
    pub group: i32,
    pub pvar_index: i32,
}

/// A camera volume: a class 2329 moby that, while the hero is in its cuboid and a moby of
/// its group lives, claims one of the follow camera's 32 volume slots and overrides some
/// of its settings (`camera_volumes_update`, 0x3F09E0). `None` leaves a setting alone.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraVolume {
    /// Which moby instance it is, and where.
    pub moby: usize,
    pub position: [f32; 3],
    /// +0x00: the cuboid the hero must be in.
    pub cuboid: Option<u32>,
    /// +0x04: the follow distance, sprung to with gain 0.001.
    pub distance: Option<f32>,
    /// +0x08: the camera's height above the look point, gain 0.001.
    pub height: Option<f32>,
    /// +0x0C: the look height above the hero, gain 0.005.
    pub look_height: Option<f32>,
    /// +0x10: the arm's tilt in radians while the right stick's y is at rest.
    pub tilt: Option<f32>,
    /// +0x14: the field of view (1.11 by default), gain 0.005.
    pub fov: Option<f32>,
    /// +0x18: how much of the hero's movement the arm takes up each frame (0.85 by
    /// default).
    pub drag: Option<f32>,
    /// +0x1C: nonzero turns on the look-point blend and stiffer look springs.
    pub stiff_look: Option<u8>,
    /// +0x1E: locks the right stick: 1 both axes, 2 yaw, 3 pitch.
    pub stick_lock: Option<i16>,
    /// +0x20: the slot's priority when nonzero (5 otherwise); the highest wins.
    pub priority: u8,
    /// +0x22: nonzero turns the arm towards `direction` every frame.
    pub steer: u8,
    /// +0x23: nonzero turns it there once the right stick has rested `idle_frames`.
    pub steer_when_idle: u8,
    /// +0x26: frames of resting stick before that steer (120 by default).
    pub idle_frames: Option<i16>,
    /// +0x30: the direction the arm is steered to.
    pub direction: Option<[f32; 3]>,
    /// +0x40: the moby group that must have a living member.
    pub group: i32,
}

/// One of the shapes the camera keeps out of, from the level's camera collision grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraCollider {
    pub centre: [f32; 3],
    pub radius: f32,
    pub kind: ShapeKind,
    /// The shape's index in the list of its kind.
    pub index: u32,
    /// Bit 0 stops the camera mode 4 from taking over while the hero is in it.
    pub flags: u32,
    pub int_value: i32,
    pub float_value: f32,
}

impl Gameplay {
    /// Reads and unpacks UYA level `level`'s gameplay file from the disc.
    pub fn read_uya(disc: &mut impl DiscReader, level: usize) -> Result<Self, FormatError> {
        let header = LevelHeader::read_uya_disc(disc, level)?;
        let at = (header.sector as u64 + header.gameplay.sector as u64) * SECTOR;
        Ok(Self {
            bytes: read_wad(disc, at, "gameplay file")?,
        })
    }

    fn image(&self) -> Image<'_> {
        Image::new(&self.bytes, 0)
    }

    /// The level's rails, in the game's order. A level without rails has none.
    ///
    /// The block is a count, the offset of the spline data (from the block's start) and
    /// its size, then one 0x20-byte record per rail, then one offset per rail into the
    /// spline data. A spline is its point count, three zero words, then the points.
    pub fn grind_paths(&self) -> Result<Vec<GrindPath>, FormatError> {
        let image = self.image();
        let block = image.u32(GRIND_PATHS)?;
        if block == 0 {
            return Ok(Vec::new());
        }
        let count = image.u32(block)?;
        let data = block.checked_add(image.u32(block + 4)?);
        let records = block + 0x10;
        let table = count
            .checked_mul(PATH_RECORD)
            .and_then(|size| records.checked_add(size));
        let (Some(data), Some(table)) = (data, table) else {
            return Err(FormatError::Invalid {
                what: "grind paths",
                address: block,
                detail: format!("{count} paths"),
            });
        };
        // Bounds the count before anything is allocated for it.
        image.bytes(table, count as usize * 4)?;
        (0..count)
            .map(|path| {
                let record = records + path * PATH_RECORD;
                let [x, y, z, radius] = image.f32s::<4>(record)?;
                let spline = offset(data, image.u32(table + path * 4)?, table + path * 4)?;
                Ok(GrindPath {
                    centre: [x, y, z],
                    radius,
                    flags: image.u32(record + 0x10)?,
                    points: read_spline(&image, spline)?
                        .into_iter()
                        .map(|[x, y, z, _]| [x, y, z])
                        .collect(),
                })
            })
            .collect()
    }

    /// The level's paths (header +0x78), each a list of points: x, y, z and a fourth
    /// float. Moby code and the path camera use them; the path camera reads a positive
    /// fourth float as a field of view in degrees to reach at that point.
    ///
    /// The block is a count, the offset of the spline data (from the block's start) and
    /// its size, then one offset per path into the spline data.
    pub fn paths(&self) -> Result<Vec<Vec<[f32; 4]>>, FormatError> {
        let image = self.image();
        let block = image.u32(PATHS)?;
        if block == 0 {
            return Ok(Vec::new());
        }
        let count = image.u32(block)?;
        let data = offset(block, image.u32(block + 4)?, block + 4)?;
        let table = block + 0x10;
        image.bytes(table, count as usize * 4)?;
        (0..count)
            .map(|path| {
                let spline = offset(data, image.u32(table + path * 4)?, table + path * 4)?;
                read_spline(&image, spline)
            })
            .collect()
    }

    /// The level's cameras (header +0x08), in the game's order, with their pvars.
    pub fn cameras(&self) -> Result<Vec<Camera>, FormatError> {
        let image = self.image();
        let Some((first, count)) = self.table(CAMERAS, CAMERA_RECORD)? else {
            return Ok(Vec::new());
        };
        (0..count)
            .map(|camera| {
                let record = first + camera * CAMERA_RECORD;
                let [x, y, z] = image.f32s::<3>(record + 4)?;
                let pvar_index = image.u32(record + 0x1C)? as i32;
                Ok(Camera {
                    class: image.u32(record)? as i32,
                    position: [x, y, z],
                    rotation: image.f32s::<3>(record + 0x10)?,
                    pvars: self.pvars(pvar_index)?.to_vec(),
                })
            })
            .collect()
    }

    /// The level's cuboids (header +0x68).
    pub fn cuboids(&self) -> Result<Vec<Shape>, FormatError> {
        self.shapes(CUBOIDS)
    }

    /// The level's spheres (header +0x6C).
    pub fn spheres(&self) -> Result<Vec<Shape>, FormatError> {
        self.shapes(SPHERES)
    }

    /// The level's cylinders (header +0x70).
    pub fn cylinders(&self) -> Result<Vec<Shape>, FormatError> {
        self.shapes(CYLINDERS)
    }

    fn shapes(&self, header: u32) -> Result<Vec<Shape>, FormatError> {
        let image = self.image();
        let Some((first, count)) = self.table(header, SHAPE_RECORD)? else {
            return Ok(Vec::new());
        };
        (0..count)
            .map(|shape| {
                let record = first + shape * SHAPE_RECORD;
                let row = |n: u32| image.f32s::<4>(record + n * 0x10);
                Ok(Shape {
                    matrix: [row(0)?, row(1)?, row(2)?, row(3)?],
                    inverse: [row(4)?, row(5)?, row(6)?],
                    rotation: image.f32s::<3>(record + 0x70)?,
                })
            })
            .collect()
    }

    /// The mobys placed in the level (header +0x4C), in the game's order.
    ///
    /// The block is the placed count, the count of mobys spawned later, two zero words,
    /// then the records, each starting with its own size.
    pub fn mobys(&self) -> Result<Vec<MobyInstance>, FormatError> {
        let image = self.image();
        let block = image.u32(MOBYS)?;
        if block == 0 {
            return Ok(Vec::new());
        }
        let count = image.u32(block)?;
        // Bounds the count before anything is allocated for it.
        image.bytes(block + 0x10, count as usize * MOBY_RECORD as usize)?;
        let mut record = block + 0x10;
        let mut mobys = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let size = image.u32(record)?;
            if size < MOBY_RECORD || size % 4 != 0 {
                return Err(FormatError::Invalid {
                    what: "moby instance",
                    address: record,
                    detail: format!("size {size:#x}"),
                });
            }
            mobys.push(MobyInstance {
                class: image.u32(record + 0x28)?,
                position: image.f32s::<3>(record + 0x40)?,
                rotation: image.f32s::<3>(record + 0x4C)?,
                group: image.u32(record + 0x58)? as i32,
                pvar_index: image.u32(record + 0x68)? as i32,
            });
            record = record.checked_add(size).ok_or(FormatError::OutOfBounds {
                address: record,
                len: size as usize,
            })?;
        }
        Ok(mobys)
    }

    /// The pvars at `index` in the pvar table (header +0x5C: an offset into the pvar data
    /// at +0x60 and a size per entry). A negative index has none.
    pub fn pvars(&self, index: i32) -> Result<&[u8], FormatError> {
        if index < 0 {
            return Ok(&[]);
        }
        let image = self.image();
        let table = image.u32(PVAR_TABLE)?;
        let entry = (index as u32)
            .checked_mul(8)
            .and_then(|at| at.checked_add(table))
            .filter(|_| table != 0)
            .ok_or(FormatError::Invalid {
                what: "pvar index",
                address: PVAR_TABLE,
                detail: format!("{index}"),
            })?;
        let start = offset(image.u32(PVAR_DATA)?, image.u32(entry)?, entry)?;
        image.bytes(start, image.u32(entry + 4)? as usize)
    }

    /// The level's camera volumes: its class 2329 mobys, read from their pvars.
    pub fn camera_volumes(&self) -> Result<Vec<CameraVolume>, FormatError> {
        let mut volumes = Vec::new();
        for (index, moby) in self.mobys()?.into_iter().enumerate() {
            if moby.class != CAMERA_VOLUME_CLASS {
                continue;
            }
            let pvars = self.pvars(moby.pvar_index)?;
            let image = Image::new(pvars, 0);
            // The game copies a field only when it isn't its "unset" value: -1.0 for the
            // floats, -1 for the halfwords, 0 for the bytes.
            let float = |at: u32| -> Result<Option<f32>, FormatError> {
                let value = image.f32(at)?;
                Ok((value != -1.0).then_some(value))
            };
            let half = |at: u32| -> Result<Option<i16>, FormatError> {
                let value = image.i16(at)?;
                Ok((value != -1).then_some(value))
            };
            let direction = image.f32s::<3>(0x30)?;
            volumes.push(CameraVolume {
                moby: index,
                position: moby.position,
                cuboid: index_of(image.u32(0x00)?),
                distance: float(0x04)?,
                height: float(0x08)?,
                look_height: float(0x0C)?,
                tilt: float(0x10)?,
                fov: float(0x14)?,
                drag: float(0x18)?,
                stiff_look: match half(0x1C)? {
                    Some(_) => Some(image.u8(0x1C)?),
                    None => None,
                },
                stick_lock: half(0x1E)?,
                priority: image.u8(0x20)?,
                steer: image.u8(0x22)?,
                steer_when_idle: image.u8(0x23)?,
                idle_frames: half(0x26)?,
                direction: (direction[0] != -1.0).then_some(direction),
                group: image.u32(0x40)? as i32,
            });
        }
        Ok(volumes)
    }

    /// The shapes in the level's camera collision grid (header +0x88), each once, in the
    /// order the grid first lists them.
    ///
    /// The block is the size of its lists, three zero words, then 64 × 64 words, one per
    /// 16 u cell from the origin (row by row in y), each 0 or a list's offset from the
    /// grid's start. A list is its count, three zero words, then 0x30-byte entries.
    pub fn camera_colliders(&self) -> Result<Vec<CameraCollider>, FormatError> {
        let image = self.image();
        let block = image.u32(CAMERA_COLLISION)?;
        if block == 0 {
            return Ok(Vec::new());
        }
        let grid = block + 0x10;
        image.bytes(grid, (GRID_CELLS * GRID_CELLS * 4) as usize)?;
        let mut seen = std::collections::BTreeSet::new();
        let mut colliders = Vec::new();
        for cell in 0..GRID_CELLS * GRID_CELLS {
            let list = image.u32(grid + cell * 4)?;
            if list == 0 {
                continue;
            }
            let list = offset(grid, list, grid + cell * 4)?;
            let count = image.u32(list)?;
            image.bytes(list + 0x10, count as usize * GRID_SHAPE as usize)?;
            for entry in 0..count {
                let at = list + 0x10 + entry * GRID_SHAPE;
                let [x, y, z, radius] = image.f32s::<4>(at)?;
                let kind = match image.u32(at + 0x10)? {
                    3 => ShapeKind::Cuboid,
                    5 => ShapeKind::Sphere,
                    6 => ShapeKind::Cylinder,
                    7 => ShapeKind::Pill,
                    other => ShapeKind::Other(other),
                };
                let index = image.u32(at + 0x14)?;
                if seen.insert((kind, index)) {
                    colliders.push(CameraCollider {
                        centre: [x, y, z],
                        radius,
                        kind,
                        index,
                        flags: image.u32(at + 0x18)?,
                        int_value: image.u32(at + 0x1C)? as i32,
                        float_value: image.f32(at + 0x20)?,
                    });
                }
            }
        }
        Ok(colliders)
    }

    /// The records of a block that starts with its count and three zero words: where the
    /// first is, and how many there are. `None` if the level has no such block.
    fn table(&self, header: u32, record: u32) -> Result<Option<(u32, u32)>, FormatError> {
        let image = self.image();
        let block = image.u32(header)?;
        if block == 0 {
            return Ok(None);
        }
        let count = image.u32(block)?;
        // Bounds the count before anything is allocated for it.
        image.bytes(block + 0x10, count as usize * record as usize)?;
        Ok(Some((block + 0x10, count)))
    }
}

impl Camera {
    /// The trigger fields every camera's pvars start with, if it has them.
    pub fn trigger(&self) -> Option<CameraTrigger> {
        let image = Image::new(&self.pvars, 0);
        let word = |at: u32| image.u32(at).ok();
        Some(CameraTrigger {
            lead: image.f32(0x00).ok()?,
            sphere: index_of(word(0x08)?),
            cuboid: index_of(word(0x0C)?),
            cylinder: index_of(word(0x10)?),
            path: index_of(word(0x14)?),
            focus_height: image.f32(0x18).ok()?,
            priority: image.u8(0x1C).ok()?,
            blend_in: image.u8(0x1D).ok()?,
            blend_out: image.u8(0x1E).ok()?,
            activation: image.u8(0x1F).ok()?,
        })
    }

    /// A path camera's own fields, if this is one with all its pvars.
    pub fn path_camera(&self) -> Option<PathCamera> {
        if self.class != PATH_CAMERA_CLASS {
            return None;
        }
        let image = Image::new(&self.pvars, 0);
        let word = |at: u32| image.u32(at).ok();
        Some(PathCamera {
            path: index_of(word(0x20)?),
            look_path: index_of(word(0x24)?),
            look_moby: index_of(word(0x28)?),
            look_offset: image.f32s::<3>(0x2C).ok()?,
            frames: image.i16(0x3A).ok()?,
            speed: image.f32(0x3C).ok()?,
            next_camera: index_of(word(0x4C)?),
            done_flag: index_of(word(0xA0)?),
        })
    }
}

impl Shape {
    /// The shape's centre.
    pub fn centre(&self) -> [f32; 3] {
        let [x, y, z, _] = self.matrix[3];
        [x, y, z]
    }

    /// The half sizes along the shape's own axes: for a sphere, its first is the radius.
    pub fn half_sizes(&self) -> [f32; 3] {
        [0, 1, 2].map(|row| {
            let [x, y, z, _] = self.matrix[row];
            (x * x + y * y + z * z).sqrt()
        })
    }

    /// `point` in the shape's own coordinates, as the game computes them.
    pub fn local(&self, point: [f32; 3]) -> [f32; 3] {
        let [cx, cy, cz] = self.centre();
        let d = [point[0] - cx, point[1] - cy, point[2] - cz];
        let mut out = [0.0; 3];
        for (axis, value) in out.iter_mut().enumerate() {
            *value = (0..3).map(|row| d[row] * self.inverse[row][axis]).sum();
        }
        out
    }

    /// Whether the game counts `point` as inside this shape as a cuboid: every local
    /// coordinate from -1 to 1 (0x440760).
    pub fn cuboid_contains(&self, point: [f32; 3]) -> bool {
        self.local(point).iter().all(|v| (-1.0..=1.0).contains(v))
    }

    /// Whether `point` is inside this shape as a sphere: nearer its centre than the
    /// length of its first axis.
    pub fn sphere_contains(&self, point: [f32; 3]) -> bool {
        let [cx, cy, cz] = self.centre();
        let d = [point[0] - cx, point[1] - cy, point[2] - cz];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < self.half_sizes()[0]
    }

    /// Whether `point` is inside this shape as a cylinder: within 1 of the local z axis,
    /// local z from -1 to 1.
    pub fn cylinder_contains(&self, point: [f32; 3]) -> bool {
        let [x, y, z] = self.local(point);
        (x * x + y * y).sqrt() < 1.0 && (-1.0..=1.0).contains(&z)
    }
}

/// An index word: -1 (or any negative) means none.
fn index_of(word: u32) -> Option<u32> {
    ((word as i32) >= 0).then_some(word)
}

/// `base + delta`, or an error naming where `delta` was read.
fn offset(base: u32, delta: u32, at: u32) -> Result<u32, FormatError> {
    base.checked_add(delta).ok_or(FormatError::OutOfBounds {
        address: at,
        len: 4,
    })
}

fn read_spline(image: &Image, at: u32) -> Result<Vec<[f32; 4]>, FormatError> {
    let count = image.u32(at)?;
    let first = at + 0x10;
    image.bytes(first, count as usize * POINT as usize)?;
    (0..count)
        .map(|point| image.f32s::<4>(first + point * POINT))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby::tests::Writer;

    /// A gameplay file whose grind path block at 0x100 holds `paths`.
    fn gameplay(paths: &[&[[f32; 3]]]) -> Gameplay {
        let mut w = Writer::default();
        w.at(GRIND_PATHS).u32(0x100);
        let records = 0x110;
        let table = records + paths.len() as u32 * PATH_RECORD;
        let data = table + paths.len() as u32 * 4;
        w.at(0x100)
            .u32(paths.len() as u32)
            .u32(data - 0x100)
            .u32(0)
            .u32(0);
        for (i, _) in paths.iter().enumerate() {
            w.f32(10.0 * i as f32).f32(2.0).f32(3.0).f32(4.0);
            w.u32(i as u32).u32(0).u32(0).u32(0);
        }
        let mut offset = 0;
        for points in paths {
            w.u32(offset);
            offset += 0x10 + points.len() as u32 * POINT;
        }
        for points in paths {
            w.u32(points.len() as u32).u32(0).u32(0).u32(0);
            for [x, y, z] in *points {
                w.f32(*x).f32(*y).f32(*z).f32(1.0);
            }
        }
        Gameplay { bytes: w.bytes }
    }

    #[test]
    fn reads_each_rails_sphere_and_points() {
        let a = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let b = [[7.0, 8.0, 9.0], [10.0, 11.0, 12.0], [13.0, 14.0, 15.0]];
        let paths = gameplay(&[&a, &b]).grind_paths().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].points, a);
        assert_eq!(paths[1].points, b);
        assert_eq!(paths[1].centre, [10.0, 2.0, 3.0]);
        assert_eq!(paths[1].radius, 4.0);
        assert_eq!(paths[1].flags, 1);
    }

    #[test]
    fn a_level_without_rails_has_none() {
        let mut w = Writer::default();
        w.at(0x100);
        let file = Gameplay { bytes: w.bytes };
        assert!(file.grind_paths().unwrap().is_empty());
        assert!(file.paths().unwrap().is_empty());
        assert!(file.cameras().unwrap().is_empty());
        assert!(file.cuboids().unwrap().is_empty());
        assert!(file.camera_volumes().unwrap().is_empty());
        assert!(file.camera_colliders().unwrap().is_empty());
    }

    #[test]
    fn a_rail_count_past_the_file_is_an_error() {
        let mut file = gameplay(&[&[[1.0, 2.0, 3.0]]]);
        file.bytes[0x100..0x104].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(file.grind_paths().is_err());
        let mut file = gameplay(&[&[[1.0, 2.0, 3.0]]]);
        let spline = file.bytes.len() - 0x20;
        file.bytes[spline..spline + 4].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(file.grind_paths().is_err());
    }

    /// Pvars for a camera of class `class` and a camera volume, two cameras, a cuboid
    /// 2 × 4 × 6 u at (10, 20, 30), two mobys (the second a camera volume), a path of
    /// three points and a camera collision grid with one cell.
    fn camera_level() -> Gameplay {
        let mut w = Writer::default();
        let header = [
            (CAMERAS, 0x100),
            (MOBYS, 0x200),
            (PVAR_TABLE, 0x400),
            (PVAR_DATA, 0x500),
            (CUBOIDS, 0x700),
            (PATHS, 0x800),
            (CAMERA_COLLISION, 0x900),
        ];
        // In order, since the writer only moves forwards.
        for (at, block) in header {
            w.at(at).u32(block);
        }
        // Cameras: a path camera with pvars 0, a follow camera without.
        w.at(0x100).u32(2).u32(0).u32(0).u32(0);
        w.u32(PATH_CAMERA_CLASS as u32)
            .f32(1.0)
            .f32(2.0)
            .f32(3.0)
            .f32(0.0)
            .f32(0.0)
            .f32(1.5)
            .u32(0);
        w.u32(0).f32(0.0).f32(0.0).f32(0.0);
        w.f32(0.0).f32(0.0).f32(0.0).u32(u32::MAX);
        // Mobys: one of class 500, then a camera volume with pvars 1.
        w.at(0x200).u32(2).u32(0).u32(0).u32(0);
        for (class, pvars) in [(500, u32::MAX), (CAMERA_VOLUME_CLASS, 1)] {
            let start = w.here();
            w.u32(MOBY_RECORD);
            w.at(start + 0x28).u32(class);
            w.at(start + 0x40).f32(5.0).f32(6.0).f32(7.0);
            w.at(start + 0x58).u32(9);
            w.at(start + 0x68).u32(pvars);
            w.at(start + MOBY_RECORD);
        }
        // The pvar table: the path camera's at 0, the volume's at 0xB0.
        w.at(0x400).u32(0).u32(0xB0).u32(0xB0).u32(0x50);
        // The path camera: no shapes, then path 1, look path 0, no moby.
        w.at(0x500).f32(0.0).u32(0).u32(u32::MAX).u32(0);
        w.u32(u32::MAX).u32(u32::MAX).f32(1.5).u32(0x0300_0006);
        w.u32(1).u32(0).u32(u32::MAX).f32(0.0);
        w.f32(0.0).f32(0.0).u16(0).u16(90).f32(0.0);
        w.at(0x500 + 0x4C).u32(u32::MAX);
        w.at(0x500 + 0xA0).u32(u32::MAX);
        // The volume: cuboid 0, distance 4, height 5, the rest unset, group 9.
        w.at(0x5B0).u32(0).f32(4.0).f32(5.0).f32(-1.0);
        w.f32(-1.0).f32(-1.0).f32(-1.0).u32(u32::MAX);
        w.u32(0).u32(u32::MAX).u32(0).u32(0);
        w.f32(-1.0).f32(-1.0).f32(-1.0).f32(-1.0);
        w.u32(9);
        // The cuboid: axes scaled to half sizes 1, 2 and 3, at (10, 20, 30).
        w.at(0x700).u32(1).u32(0).u32(0).u32(0);
        w.f32(1.0).f32(0.0).f32(0.0).f32(0.0);
        w.f32(0.0).f32(2.0).f32(0.0).f32(0.0);
        w.f32(0.0).f32(0.0).f32(3.0).f32(0.0);
        w.f32(10.0).f32(20.0).f32(30.0).f32(1.0);
        w.f32(1.0).f32(0.0).f32(0.0).f32(0.0);
        w.f32(0.0).f32(0.5).f32(0.0).f32(0.0);
        w.f32(0.0).f32(0.0).f32(1.0 / 3.0).f32(0.0);
        w.f32(0.0).f32(0.0).f32(0.0).f32(0.0);
        // Paths: two, the second with three points whose fourth floats are degrees.
        w.at(0x800).u32(2).u32(0x20).u32(0x70).u32(0);
        w.u32(0).u32(0x20);
        w.at(0x820).u32(1).u32(0).u32(0).u32(0);
        w.f32(0.0).f32(0.0).f32(0.0).f32(0.0);
        w.u32(3).u32(0).u32(0).u32(0);
        for x in 0..3 {
            w.f32(x as f32).f32(1.0).f32(2.0).f32(60.0);
        }
        // The camera collision grid: cell (1, 2) lists the cuboid twice.
        w.at(0x900).u32(0x80).u32(0).u32(0).u32(0);
        let grid = 0x910;
        w.at(grid + (2 * GRID_CELLS + 1) * 4).u32(0x4000);
        w.at(grid + 0x4000).u32(2).u32(0).u32(0).u32(0);
        for _ in 0..2 {
            w.f32(10.0).f32(20.0).f32(30.0).f32(3.8);
            w.u32(3).u32(0).u32(1).u32(0);
            w.f32(0.0).u32(0).u32(0).u32(0);
        }
        Gameplay { bytes: w.bytes }
    }

    #[test]
    fn reads_cameras_with_their_trigger_and_path_fields() {
        let cameras = camera_level().cameras().unwrap();
        assert_eq!(cameras.len(), 2);
        assert_eq!(cameras[0].class, PATH_CAMERA_CLASS);
        assert_eq!(cameras[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(cameras[0].rotation, [0.0, 0.0, 1.5]);
        let trigger = cameras[0].trigger().unwrap();
        assert_eq!(trigger.priority, 6);
        assert_eq!(trigger.blend_in, 0);
        assert_eq!(trigger.activation, 3);
        assert_eq!(trigger.cuboid, Some(0));
        assert_eq!(trigger.sphere, None);
        assert_eq!(trigger.focus_height, 1.5);
        let path = cameras[0].path_camera().unwrap();
        assert_eq!(path.path, Some(1));
        assert_eq!(path.look_path, Some(0));
        assert_eq!(path.look_moby, None);
        assert_eq!(path.frames, 90);
        assert_eq!(path.next_camera, None);
        // The second has no pvars, so no trigger.
        assert!(cameras[1].pvars.is_empty());
        assert_eq!(cameras[1].trigger(), None);
        assert_eq!(cameras[1].path_camera(), None);
    }

    #[test]
    fn reads_camera_volumes_from_their_mobys() {
        let level = camera_level();
        let mobys = level.mobys().unwrap();
        assert_eq!(mobys.len(), 2);
        assert_eq!(mobys[1].group, 9);
        let volumes = level.camera_volumes().unwrap();
        assert_eq!(volumes.len(), 1);
        let volume = &volumes[0];
        assert_eq!(volume.moby, 1);
        assert_eq!(volume.position, [5.0, 6.0, 7.0]);
        assert_eq!(volume.cuboid, Some(0));
        assert_eq!(volume.distance, Some(4.0));
        assert_eq!(volume.height, Some(5.0));
        assert_eq!(volume.look_height, None);
        assert_eq!(volume.fov, None);
        assert_eq!(volume.stiff_look, None);
        assert_eq!(volume.stick_lock, None);
        assert_eq!(volume.direction, None);
        assert_eq!(volume.group, 9);
    }

    #[test]
    fn tests_points_against_a_cuboid_as_the_game_does() {
        let cuboids = camera_level().cuboids().unwrap();
        let cuboid = &cuboids[0];
        assert_eq!(cuboid.centre(), [10.0, 20.0, 30.0]);
        assert_eq!(cuboid.half_sizes(), [1.0, 2.0, 3.0]);
        assert!(cuboid.cuboid_contains([10.5, 21.9, 27.1]));
        assert!(!cuboid.cuboid_contains([10.5, 22.1, 30.0]));
        assert!(!cuboid.cuboid_contains([8.9, 20.0, 30.0]));
        assert!(cuboid.sphere_contains([10.0, 20.0, 30.9]));
        assert!(cuboid.cylinder_contains([10.0, 20.0, 32.9]));
        assert!(!cuboid.cylinder_contains([10.0, 22.0, 30.0]));
    }

    #[test]
    fn reads_paths_with_their_fourth_floats() {
        let paths = camera_level().paths().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], [[0.0; 4]]);
        assert_eq!(paths[1].len(), 3);
        assert_eq!(paths[1][2], [2.0, 1.0, 2.0, 60.0]);
    }

    #[test]
    fn lists_each_camera_collider_once() {
        let colliders = camera_level().camera_colliders().unwrap();
        assert_eq!(colliders.len(), 1);
        assert_eq!(colliders[0].kind, ShapeKind::Cuboid);
        assert_eq!(colliders[0].index, 0);
        assert_eq!(colliders[0].flags, 1);
        assert_eq!(colliders[0].radius, 3.8);
    }

    #[test]
    fn bad_camera_data_is_an_error() {
        let mut level = camera_level();
        level.bytes[0x100..0x104].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(level.cameras().is_err());
        let mut level = camera_level();
        level.bytes[0x210..0x214].copy_from_slice(&4u32.to_le_bytes());
        assert!(level.mobys().is_err());
        let mut level = camera_level();
        level.bytes[0x404..0x408].copy_from_slice(&0x1000_0000u32.to_le_bytes());
        assert!(level.cameras().is_err());
        let mut level = camera_level();
        level.bytes[0x810..0x814].copy_from_slice(&0x7FFF_FFF0u32.to_le_bytes());
        assert!(level.paths().is_err());
        let mut level = camera_level();
        let cell = (0x910 + (2 * GRID_CELLS + 1) * 4) as usize;
        level.bytes[cell..cell + 4].copy_from_slice(&0x0100_0000u32.to_le_bytes());
        assert!(level.camera_colliders().is_err());
    }
}

/// Reads the real disc: `OPENRAC_SOURCE_DIR` names the folder of the user's disc images.
#[cfg(test)]
mod disc_tests {
    use super::*;
    use crate::level::disc_tests::disc;

    /// Veldin's cameras and its one camera volume, as the game loaded them in our Veldin
    /// savestates (`docs/research/gameplay-format.md`, "Cameras").
    #[test]
    #[ignore = "needs game discs"]
    fn veldins_cameras_read_as_the_game_loads_them() {
        let mut disc = disc(rac_iso::Release::UyaNtsc);
        let veldin = Gameplay::read_uya(&mut disc, 1).unwrap();
        let classes: Vec<i32> = veldin.cameras().unwrap().iter().map(|c| c.class).collect();
        assert_eq!(classes, [0x17, 0x13, 0, 4, 5, 0x20, 7]);
        let cameras = veldin.cameras().unwrap();
        let fixed = cameras[0].trigger().unwrap();
        assert_eq!(
            (fixed.cuboid, fixed.priority, fixed.activation),
            (Some(43), 4, 3)
        );
        let path = cameras[1].path_camera().unwrap();
        assert_eq!((path.path, path.look_path), (Some(151), Some(152)));
        assert_eq!(veldin.paths().unwrap().len(), 159);
        assert_eq!(veldin.cuboids().unwrap().len(), 125);
        let volumes = veldin.camera_volumes().unwrap();
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].cuboid, Some(63));
        assert_eq!(volumes[0].distance, Some(4.0));
        assert_eq!(volumes[0].height, Some(5.0));
        assert_eq!(volumes[0].group, 51);
    }
}
