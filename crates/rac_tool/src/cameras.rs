//! `openrac-tool cameras`: a UYA level's cameras, camera volumes, shapes and paths from
//! its gameplay file (`docs/research/gameplay-format.md`), optionally checked against
//! what the game loaded in an EE memory image of the same level.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rac_formats::Image;
use rac_formats::gameplay::{Camera, CameraVolume, Gameplay, Shape};
use rac_formats::level::Toc;

use crate::open_disc;

pub fn run(iso: &Path, level: Option<usize>, check_ee: Option<&Path>) -> Result<()> {
    let mut disc = open_disc(iso)?;
    let levels = match level {
        Some(level) => vec![level],
        None => (0..Toc::read_uya_disc(&mut disc)?.levels.len()).collect(),
    };
    for number in levels {
        let gameplay = match Gameplay::read_uya(&mut disc, number) {
            Ok(gameplay) => gameplay,
            // Gaps in the level table.
            Err(_) if level.is_none() => continue,
            Err(error) => return Err(error.into()),
        };
        describe(number, &gameplay, level.is_some())
            .with_context(|| format!("reading level {number}'s cameras"))?;
        if let Some(ee) = check_ee {
            check(&gameplay, ee)?;
        }
    }
    Ok(())
}

fn describe(level: usize, gameplay: &Gameplay, details: bool) -> Result<()> {
    let cameras = gameplay.cameras()?;
    let volumes = gameplay.camera_volumes()?;
    let paths = gameplay.paths()?;
    let cuboids = gameplay.cuboids()?;
    let spheres = gameplay.spheres()?;
    let cylinders = gameplay.cylinders()?;
    let colliders = gameplay.camera_colliders()?;
    let classes: Vec<String> = cameras.iter().map(|c| format!("{:#x}", c.class)).collect();
    let mut kinds = BTreeMap::new();
    for collider in &colliders {
        *kinds.entry(collider.kind).or_insert(0) += 1;
    }
    println!(
        "level {level}: {} cameras [{}], {} camera volumes, {} paths, {} cuboids, {} spheres, \
         {} cylinders, {} camera colliders {kinds:?}",
        cameras.len(),
        classes.join(" "),
        volumes.len(),
        paths.len(),
        cuboids.len(),
        spheres.len(),
        cylinders.len(),
        colliders.len(),
    );
    if !details {
        return Ok(());
    }
    for (index, camera) in cameras.iter().enumerate() {
        describe_camera(index, camera, &cuboids, &paths);
    }
    for volume in &volumes {
        describe_volume(volume, &cuboids);
    }
    for collider in &colliders {
        println!(
            "  collider: {:?} {} at {:.2?} r {:.2}, flags {:#x}, values {} {}",
            collider.kind,
            collider.index,
            collider.centre,
            collider.radius,
            collider.flags,
            collider.int_value,
            collider.float_value
        );
    }
    Ok(())
}

fn describe_camera(index: usize, camera: &Camera, cuboids: &[Shape], paths: &[Vec<[f32; 4]>]) {
    println!(
        "  camera {index}: class {:#x} at {:.2?}, rotation {:.3?}, {} bytes of pvars",
        camera.class,
        camera.position,
        camera.rotation,
        camera.pvars.len()
    );
    if let Some(trigger) = camera.trigger() {
        println!(
            "    priority {}, blend in {} out {}, activation {}, lead {}, focus height {}",
            trigger.priority,
            trigger.blend_in,
            trigger.blend_out,
            trigger.activation,
            trigger.lead,
            trigger.focus_height
        );
        let shapes = [
            ("sphere", trigger.sphere),
            ("cuboid", trigger.cuboid),
            ("cylinder", trigger.cylinder),
            ("path", trigger.path),
        ];
        for (kind, index) in shapes {
            if let Some(index) = index {
                println!("    {kind} {index}");
            }
        }
        if let Some(cuboid) = trigger.cuboid.and_then(|i| cuboids.get(i as usize)) {
            describe_shape(cuboid);
        }
    }
    if let Some(path) = camera.path_camera() {
        println!(
            "    flies along path {:?} looking at path {:?} or moby {:?} (+{:.2?}), {} frames \
             or {:.2} u/s, then camera {:?}; done flag {:?}",
            path.path,
            path.look_path,
            path.look_moby,
            path.look_offset,
            path.frames,
            path.speed,
            path.next_camera,
            path.done_flag
        );
        for (role, index) in [("path", path.path), ("look path", path.look_path)] {
            let Some(points) = index.and_then(|i| paths.get(i as usize)) else {
                continue;
            };
            println!("    {role} {}: {} points", index.unwrap_or(0), points.len());
            for point in points {
                println!("      {point:.2?}");
            }
        }
    }
    if camera.pvars.len() > 0x20 {
        let words: Vec<String> = camera.pvars[0x20..]
            .chunks(4)
            .map(|word| {
                let mut bytes = [0; 4];
                bytes[..word.len()].copy_from_slice(word);
                format!("{:08x}", u32::from_le_bytes(bytes))
            })
            .collect();
        println!("    pvars from +0x20: {}", words.join(" "));
    }
}

fn describe_volume(volume: &CameraVolume, cuboids: &[Shape]) {
    println!(
        "  camera volume (moby {} at {:.2?}): cuboid {:?}, group {}, priority {}",
        volume.moby, volume.position, volume.cuboid, volume.group, volume.priority
    );
    let settings = [
        ("distance", volume.distance),
        ("height", volume.height),
        ("look height", volume.look_height),
        ("tilt", volume.tilt),
        ("fov", volume.fov),
        ("drag", volume.drag),
    ];
    let set: Vec<String> = settings
        .iter()
        .filter_map(|(name, value)| value.map(|value| format!("{name} {value}")))
        .collect();
    println!(
        "    {}; stiff look {:?}, stick lock {:?}, steer {} / when idle {} ({:?} frames) \
         towards {:.3?}",
        set.join(", "),
        volume.stiff_look,
        volume.stick_lock,
        volume.steer,
        volume.steer_when_idle,
        volume.idle_frames,
        volume.direction
    );
    if let Some(cuboid) = volume.cuboid.and_then(|i| cuboids.get(i as usize)) {
        describe_shape(cuboid);
    }
}

fn describe_shape(shape: &Shape) {
    println!(
        "      centre {:.2?}, half sizes {:.2?}, rotation {:.3?}",
        shape.centre(),
        shape.half_sizes(),
        shape.rotation
    );
}

/// Finds the level's cameras, cuboids and camera volumes in an EE memory image of the same
/// level, by their contents, and says where each differs from the disc.
fn check(gameplay: &Gameplay, ee: &Path) -> Result<()> {
    let bytes = std::fs::read(ee).with_context(|| format!("reading {}", ee.display()))?;
    let image = Image::ee(&bytes);
    let cuboids = gameplay.cuboids()?;
    if let Some(first) = cuboids.first() {
        // Old copies of the file can linger in memory, so take the copy that matches best.
        let matching = |address: u32| {
            cuboids
                .iter()
                .enumerate()
                .filter(|(i, cuboid)| {
                    image
                        .bytes(address + *i as u32 * 0x80, 0x7C)
                        .is_ok_and(|loaded| loaded == shape_bytes(cuboid))
                })
                .count()
        };
        let (address, same) = find_all(&bytes, &shape_bytes(first))
            .map(|address| (address, matching(address)))
            .max_by_key(|&(_, same)| same)
            .context("the first cuboid isn't loaded")?;
        println!(
            "  cuboids at {address:#x}: {same} of {} as on the disc",
            cuboids.len()
        );
    }
    let cameras = gameplay.cameras()?;
    if let Some(first) = cameras.first() {
        // The loader copies each record as position, class, rotation, pvar pointer.
        let mut key = Vec::new();
        for value in first.position {
            key.extend_from_slice(&value.to_le_bytes());
        }
        key.extend_from_slice(&first.class.to_le_bytes());
        let list = find(&bytes, &key).context("the first camera isn't loaded")?;
        println!("  cameras at {list:#x}:");
        for (index, camera) in cameras.iter().enumerate() {
            let record = list + index as u32 * 0x20;
            let class = image.u32(record + 0xC)? as i32;
            let pvars = image.u32(record + 0x1C)?;
            if class != camera.class {
                bail!("camera {index} is class {class:#x} in memory");
            }
            let loaded = image.bytes(pvars, camera.pvars.len())?;
            // +0x04 is the pointer to the camera's update object, filled in at load.
            let differ: Vec<String> = (0..camera.pvars.len() / 4)
                .filter(|word| {
                    *word != 1 && loaded[word * 4..word * 4 + 4] != camera.pvars[word * 4..][..4]
                })
                .map(|word| format!("+{:#x}", word * 4))
                .collect();
            println!(
                "    camera {index} (class {class:#x}), pvars at {pvars:#x}: {}",
                if differ.is_empty() {
                    "as on the disc".to_string()
                } else {
                    format!("words {} changed since loading", differ.join(" "))
                }
            );
        }
    }
    for volume in gameplay.camera_volumes()? {
        let moby = gameplay.mobys()?[volume.moby].clone();
        let pvars = gameplay.pvars(moby.pvar_index)?;
        // +0x28 and +0x2C are the volume's state, set while it runs.
        let key = &pvars[..0x28.min(pvars.len())];
        match find(&bytes, key) {
            Some(address) => println!(
                "  camera volume (moby {}): pvars at {address:#x} as on the disc; in use {}, slot \
                 {:#x}",
                volume.moby,
                image.u32(address + 0x28)?,
                image.u32(address + 0x2C)?
            ),
            None => println!(
                "  camera volume (moby {}): pvars not found in memory",
                volume.moby
            ),
        }
    }
    Ok(())
}

/// A shape's record as the loader copies it, but for its unused last word.
fn shape_bytes(shape: &Shape) -> Vec<u8> {
    let mut out = Vec::with_capacity(0x7C);
    for row in shape.matrix.iter().chain(&shape.inverse) {
        for value in row {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for value in shape.rotation {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// The address of the first 16-byte-aligned copy of `key` in an EE image.
fn find(bytes: &[u8], key: &[u8]) -> Option<u32> {
    find_all(bytes, key).next()
}

/// The addresses of every 16-byte-aligned copy of `key` in an EE image.
fn find_all<'a>(bytes: &'a [u8], key: &'a [u8]) -> impl Iterator<Item = u32> + 'a {
    (0..bytes.len().saturating_sub(key.len()))
        .step_by(16)
        .filter(move |&at| !key.is_empty() && bytes[at..at + key.len()] == *key)
        .map(|at| at as u32)
}
