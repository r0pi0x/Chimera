//! Development tool: inspect and extract a game install's files.
//! See docs/OBSERVABILITY.md ("Exploring game files").
//!
//! Dev tool: summarize, list and extract a game install's files; `--sequences`
//! lists a model's bones and sequences.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use bevy::math::{Quat, Vec3};
use hl2dm::game as hl2dm;
use lithtech_formats::rez;
use mashup::{
    games::cs_source,
    mount::{
        Mount,
        config::{LocalConfig, default_dump_dir},
    },
};

const USAGE: &str = "\
usage: dump <game> [options]
  games: cs_source, combat_arms, hl2dm
  (no option)          summary: file counts and sizes by extension (hl2dm: its
                       search path from gameinfo.txt first, each entry and whether it's there)
  --list               list every file with its size
  --archives           combat_arms: per-archive title, entropy and file count
  --sequences <model>  cs_source: a model's bones, sequences (activity, frames, fps,
                       duration, looping, ground speed) and pose parameters, includes merged
  --lods <model|text>  cs_source: each model's LODs (switch point, triangles) from its .dx90.vtx;
                       with text, every .mdl whose path contains it
  --objects <map>      combat_arms: a world file's objects (class, every property) and
                       its world models' boxes; --filter keeps objects whose class,
                       name or a property value contains <text>
  --filter <text>      only paths containing <text> (case-insensitive)
  --extract            write the (filtered) files to --out
  --out <dir>          extraction folder (default: per-user data dir; never inside the repo)
  --install <dir>      install folder (default: from chimera.local.toml)";

struct Args {
    game: String,
    list: bool,
    archives: bool,
    extract: bool,
    filter: Option<String>,
    sequences: Option<String>,
    lods: Option<String>,
    objects: Option<String>,
    out: Option<PathBuf>,
    install: Option<PathBuf>,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let game = it.next().ok_or("missing <game>")?;
    if game == "--help" || game == "-h" {
        return Err(String::new());
    }
    let mut a = Args {
        game,
        list: false,
        archives: false,
        extract: false,
        filter: None,
        sequences: None,
        lods: None,
        objects: None,
        out: None,
        install: None,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag}: missing value"));
        match flag.as_str() {
            "--list" => a.list = true,
            "--archives" => a.archives = true,
            "--extract" => a.extract = true,
            "--filter" => a.filter = Some(value()?.to_lowercase()),
            "--sequences" => a.sequences = Some(value()?.to_lowercase().replace('\\', "/")),
            "--lods" => a.lods = Some(value()?.to_lowercase().replace('\\', "/")),
            "--objects" => a.objects = Some(value()?.to_lowercase()),
            "--out" => a.out = Some(value()?.into()),
            "--install" => a.install = Some(value()?.into()),
            _ => return Err(format!("unknown option {flag}")),
        }
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}\n");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn install_path(args: &Args) -> Result<PathBuf, String> {
    if let Some(p) = &args.install {
        return Ok(p.clone());
    }
    let steam = match args.game.as_str() {
        cs_source::GAME => cs_source::INFO.steam,
        hl2dm::GAME => hl2dm::INFO.steam,
        _ => None,
    };
    LocalConfig::load()?.game_path_in(&args.game, steam.as_ref()).ok_or_else(|| {
        format!(
            "no install path for `{}`. Set [games.{}] path in chimera.local.toml \
             (copy chimera.local.example.toml) or pass --install.",
            args.game, args.game
        )
    })
}

fn run(args: &Args) -> Result<(), String> {
    let install = install_path(args)?;
    match args.game.as_str() {
        cs_source::GAME => dump_cs_source(args, &install),
        COMBAT_ARMS => dump_combat_arms(args, &install),
        hl2dm::GAME => dump_hl2dm(args, &install),
        other => Err(format!("unknown game `{other}`; expected cs_source, combat_arms or hl2dm")),
    }
}

/// HL2:DM: the search path its `gameinfo.txt` gives (when the install's
/// own folders are there), then the mount's summary.
fn dump_hl2dm(args: &Args, install: &Path) -> Result<(), String> {
    if let Ok(path) = hl2dm::mount::search_path(install) {
        println!("gameinfo.txt search path (first wins):");
        for layer in &path.layers {
            println!("  {layer}");
        }
    }
    let mount = hl2dm::mount::open(install).map_err(|e| e.to_string())?;
    report(args, &mount, &[])
}

fn human(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

fn dump_cs_source(args: &Args, install: &Path) -> Result<(), String> {
    let mount = cs_source::mount::open(install).map_err(|e| e.to_string())?;
    if let Some(path) = &args.sequences {
        return sequences(&mount, path);
    }
    if let Some(filter) = &args.lods {
        return lods(&mount, filter);
    }
    report(args, &mount, &[])
}

/// Combat Arms' name in the local config (`[games.combat_arms]`).
const COMBAT_ARMS: &str = "combat_arms";

/// Combat Arms' keys (`[games.combat_arms.keys.<scheme>]` in the local
/// config; never in the repo), by scheme.
fn combat_arms_keys() -> Result<rez::RezKeys, String> {
    let config = LocalConfig::load()?;
    let mut keys = rez::RezKeys::new();
    for (scheme, k) in config.game_keys(COMBAT_ARMS).into_iter().flatten() {
        let cipher = rez::RezCipher::from_hex(scheme, &k.key, &k.iv, &format!("{COMBAT_ARMS} {scheme}"))?;
        keys.insert(scheme.clone(), cipher);
    }
    Ok(keys)
}

/// Every archive in the install's `Game/` folder (lithtech_formats' `.rez`
/// reader).
fn dump_combat_arms(args: &Args, install: &Path) -> Result<(), String> {
    let keys = combat_arms_keys()?;
    let game = install.join("Game");
    let (mount, locked) =
        rez::open_dir(&game, &keys).map_err(|e| format!("{e}; is {} a Combat Arms install?", install.display()))?;
    if let Some(map) = &args.objects {
        return world_objects(&mount, map, args.filter.as_deref());
    }
    if args.archives {
        println!("{:>10}  {:7}  {:16}  {:>6}  name", "size", "entropy", "title", "files");
        for layer in mount.layers() {
            let path = PathBuf::from(layer.name());
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let archive = rez::RezArchive::open(&path, &keys).map_err(|e| e.to_string())?;
            let entropy = sample_entropy(&path, 1 << 20).map_err(|e| e.to_string())?;
            let title: String = archive.header().title.chars().take(16).collect();
            let name = path.file_name().unwrap().to_string_lossy();
            println!(
                "{:>10}  {entropy:7.3}  {title:16}  {:>6}  {name}",
                human(size),
                layer.entries().len()
            );
        }
    }
    report(args, &mount, &locked)
}

/// A Combat Arms world file's objects (class and properties, in file
/// order) and the boxes and centres of its world models (file axes),
/// read in memory: numbers and structure for checking specs' cases.
fn world_objects(mount: &Mount, map: &str, filter: Option<&str>) -> Result<(), String> {
    use lithtech_formats::world::{Value, World};
    let file = format!("/{map}.dat");
    let path = mount
        .entries()
        .into_iter()
        .map(|(e, _)| e.path)
        .filter(|p| p.starts_with("worlds") && p.ends_with(&file))
        .min()
        .ok_or_else(|| format!("{map}: no worlds*/{map}.dat"))?;
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let world = World::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let text = |v: &Value| match v {
        Value::String(s) => format!("{s:?}"),
        Value::Vector(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
        Value::Color(c) => format!("rgb({}, {}, {})", c[0], c[1], c[2]),
        Value::Real(x) => format!("{x}"),
        Value::Flags(x) => format!("flags {x}"),
        Value::Long(x) => format!("long {x}"),
        Value::Bool(b) => format!("{b}"),
        Value::Rotation(r) => format!("rot({}, {}, {})", r[0], r[1], r[2]),
        Value::Unknown { code, data } => format!("unknown {code} ({} bytes)", data.len()),
    };
    let wanted = |s: &str| filter.is_none_or(|f| s.to_lowercase().contains(f));
    println!("{path}: {} objects, {} world models, {} blind chunks", world.objects.len(), world.models.len(), world.blind_data.len());
    for (i, o) in world.objects.iter().enumerate() {
        let props: Vec<String> = o.properties.iter().map(|p| format!("{} = {}", p.name, text(&p.value))).collect();
        if !(wanted(&o.class) || props.iter().any(|p| wanted(p))) {
            continue;
        }
        println!("[{i}] {}", o.class);
        for p in props {
            println!("    {p}");
        }
        if let Some(m) = o.name().and_then(|n| world.models.iter().find(|m| m.name == n)) {
            println!(
                "    (world model: min {:?} max {:?} translation {:?}, {} polygons)",
                m.min, m.max, m.translation, m.polygons.len()
            );
        }
    }
    for (i, c) in world.blind_data.iter().enumerate() {
        if filter.is_none() {
            println!("blind chunk {i}: id {:#010x}, {} bytes", c.id, c.data.len());
        }
    }
    Ok(())
}

/// Shannon entropy in bits per byte of up to `len` bytes from the middle
/// of a file: close to 8.0 means encrypted or compressed.
fn sample_entropy(path: &Path, len: usize) -> std::io::Result<f64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path)?;
    let size = f.metadata()?.len();
    f.seek(SeekFrom::Start(size.saturating_sub(len as u64) / 2))?;
    let mut buf = Vec::with_capacity(len);
    f.take(len as u64).read_to_end(&mut buf)?;
    let mut counts = [0u64; 256];
    for &b in &buf {
        counts[b as usize] += 1;
    }
    let n = buf.len() as f64;
    Ok(counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum())
}

/// A model's skeleton and what it can play (our own `.mdl` decoder).
/// Each model's LODs: per body part model, each LOD's switch point and
/// triangle count (`r_rootlod`, distance LOD switching).
fn lods(mount: &Mount, filter: &str) -> Result<(), String> {
    let paths: Vec<String> = if filter.ends_with(".mdl") {
        vec![filter.to_string()]
    } else {
        mount
            .entries()
            .into_iter()
            .map(|(e, _)| e.path)
            .filter(|p| p.ends_with(".mdl") && p.contains(filter))
            .collect()
    };
    for path in paths {
        let stem = path.trim_end_matches(".mdl");
        let Some(bytes) = mount.read(&format!("{stem}.dx90.vtx")).ok() else {
            println!("{path}: no .dx90.vtx");
            continue;
        };
        let vtx = vmdl::vtx::Vtx::read(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let mut line = format!("{path}:");
        for (p, part) in vtx.body_parts.iter().enumerate() {
            for (m, model) in part.models.iter().enumerate() {
                let lods: Vec<String> = model
                    .lods
                    .iter()
                    .map(|lod| {
                        let tris: usize = lod
                            .meshes
                            .iter()
                            .flat_map(|mesh| &mesh.strip_groups)
                            .map(|g| g.indices.len() / 3)
                            .sum();
                        format!("{:.1}:{tris}", lod.switch_point)
                    })
                    .collect();
                line += &format!(" [{p}.{m}] {}", lods.join(" "));
            }
        }
        println!("{line}");
    }
    Ok(())
}

fn sequences(mount: &Mount, path: &str) -> Result<(), String> {
    let read = |p: &str| mount.read(&p.to_lowercase().replace('\\', "/")).ok();
    let bones = cs_source::anim::bones(&read, path)?;
    let set = cs_source::anim::load(&read, path)?;
    println!(
        "{path}: {} bones, {} animations, {} sequences",
        bones.len(),
        set.animations.len(),
        set.sequences.len()
    );
    for (i, (name, parent, _, _)) in bones.iter().enumerate() {
        println!("  bone {i:>3} {name} (parent {parent:?})");
    }
    for p in &set.params {
        println!("  param {} {}..{} (wrap {})", p.name, p.start, p.end, p.looping);
    }
    for (i, s) in set.sequences.iter().enumerate() {
        let a = &set.animations[s.anims[0]];
        let duration = if a.frames > 1 {
            (a.frames - 1) as f32 / a.fps
        } else {
            0.0
        };
        println!(
            "  seq {i:>3} {:24} {:28} w{:<3} {:>4} frames {:>5.1} fps {duration:>7.4} s{}{}",
            s.name,
            s.activity,
            s.activity_weight,
            a.frames,
            a.fps,
            if s.looping { " looping" } else { "" },
            if a.speed > 0.0 {
                format!(" {:.1} u/s", a.speed)
            } else {
                String::new()
            }
        );
        for e in &s.events {
            println!("        event {:.4} {} {:?} {:?}", e.cycle, e.event, e.name, e.options);
        }
        if let Some(m) = motion(&set, i, &bones) {
            println!("        {m}");
        }
    }
    let (attachments, illum) = cs_source::anim::attachments(&read, path)?;
    for (name, bone, t) in attachments {
        let (x, y, z) = (t.rotation * Vec3::X, t.rotation * Vec3::Y, t.rotation * Vec3::Z);
        println!(
            "  attachment {name:?} bone {bone} at {} axes x {x} y {y} z {z}",
            t.translation
        );
    }
    println!("  illumination position {illum}");
    Ok(())
}

/// How much a sequence moves: the largest turn of any bone (degrees,
/// its local rotation against the first frame's) and the cycles over which
/// some bone is turned more than 2 degrees; and for an idle, how far its
/// first frame is from the end of the draw sequence (a still idle that
/// starts where the draw ends looks like no idle at all).
fn motion(set: &mashup::map::anim::AnimSet, s: usize, bones: &[(String, Option<usize>, Quat, Vec3)]) -> Option<String> {
    let params = set.default_params();
    let frames = set.animations[set.sequences[s].anims[0]].frames.max(2);
    let n = set.defaults.len();
    let pose = |s: usize, cycle: f32| {
        let mut out = set.defaults.clone();
        set.sequence_pose(s, cycle, &params, &mut out[..n]);
        out
    };
    let differ = |a: &[(Quat, Vec3)], b: &[(Quat, Vec3)]| {
        a.iter()
            .zip(b)
            .enumerate()
            .map(|(i, (p, q))| (p.0.angle_between(q.0).to_degrees(), i))
            .fold((0.0f32, 0usize), |m, x| if x.0 > m.0 { x } else { m })
    };
    let first = pose(s, 0.0);
    let (mut max, mut bone, mut span) = (0.0f32, 0usize, None::<(f32, f32)>);
    for f in 0..frames {
        let c = f as f32 / (frames - 1) as f32;
        let (d, b) = differ(&pose(s, c), &first);
        if d > max {
            (max, bone) = (d, b);
        }
        if d > 2.0 {
            span = Some(span.map_or((c, c), |(a, _)| (a, c)));
        }
    }
    let name = |b: usize| bones.get(b).map_or("?", |x| x.0.as_str()).to_string();
    let mut line = format!("motion: up to {max:.1} deg ({})", name(bone));
    if let Some((a, b)) = span {
        line += &format!(", over 2 deg in cycles {a:.2}..{b:.2}");
    }
    let seq = &set.sequences[s];
    if seq.activity.eq_ignore_ascii_case("ACT_VM_IDLE")
        && let Some(draw) = set.activity("ACT_VM_DRAW")
    {
        let (d, b) = differ(&first, &pose(draw, 1.0));
        line += &format!("; first frame vs the draw's end: {d:.1} deg ({})", name(b));
    }
    Some(line)
}

fn report(args: &Args, mount: &Mount, locked: &[String]) -> Result<(), String> {
    let layers: Vec<String> = mount.layers().map(|l| l.name()).collect();
    let entries: Vec<_> = mount
        .entries()
        .into_iter()
        .filter(|(e, _)| args.filter.as_ref().is_none_or(|f| e.path.contains(f.as_str())))
        .collect();

    if args.list {
        for (e, layer) in &entries {
            let archive = Path::new(&layers[*layer])
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            println!("{:>10}  {}  [{archive}]", e.size, e.path);
        }
    }

    let mut by_ext: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for (e, _) in &entries {
        let ext = e.path.rsplit_once('.').map(|(_, x)| x).unwrap_or("(none)").to_string();
        let slot = by_ext.entry(ext).or_default();
        slot.0 += 1;
        slot.1 += e.size;
    }
    if layers.len() <= 16 {
        println!("search path:");
        for (i, l) in layers.iter().enumerate() {
            println!("  [{i}] {l}");
        }
    } else {
        println!("search path: {} layers (--archives for details)", layers.len());
    }
    let total: u64 = entries.iter().map(|(e, _)| e.size).sum();
    println!("{} files, {}", entries.len(), human(total));
    let mut top: Vec<_> = by_ext.into_iter().collect();
    top.sort_by_key(|(_, (_, size))| std::cmp::Reverse(*size));
    for (ext, (count, size)) in top.iter().take(15) {
        println!("  {ext:>8}: {count:>6} files, {}", human(*size));
    }
    if !locked.is_empty() {
        println!(
            "{} archives are encrypted and have no key configured; their files list but won't extract: {}",
            locked.len(),
            locked.join(", ")
        );
    }

    if args.extract {
        let out = out_dir(args)?;
        let (mut written, mut skipped) = (0, 0);
        for (e, _) in &entries {
            let data = match mount.read(&e.path) {
                Ok(d) => d,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    skipped += 1;
                    continue;
                }
                Err(err) => return Err(format!("{}: {err}", e.path)),
            };
            let dest = out.join(&e.path);
            fs::create_dir_all(dest.parent().unwrap()).map_err(|err| format!("{}: {err}", dest.display()))?;
            fs::write(&dest, data).map_err(|err| format!("{}: {err}", dest.display()))?;
            written += 1;
        }
        println!("extracted {written} files to {}", out.display());
        if skipped > 0 {
            println!("skipped {skipped} encrypted files (no key configured)");
        }
    }
    Ok(())
}

/// The extraction folder. Refuses anything inside this repository, so game
/// files can't end up in git by accident.
fn out_dir(args: &Args) -> Result<PathBuf, String> {
    let out = match &args.out {
        Some(p) => p.clone(),
        None => default_dump_dir(&args.game).ok_or("no per-user data folder; pass --out")?,
    };
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = out.canonicalize().map_err(|e| e.to_string())?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if out.starts_with(&repo) {
        return Err(format!(
            "{} is inside the repository. Game files must never be stored in the repo; choose a folder outside it.",
            out.display()
        ));
    }
    Ok(out)
}
