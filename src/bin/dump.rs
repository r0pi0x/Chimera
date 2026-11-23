//! Development tool: inspect and extract a game install's files.
//! See docs/OBSERVABILITY.md ("Exploring game files").
//!
//! Dev tool: summarize, list and extract a game install's files; `--sequences`
//! lists a model's bones and sequences (CS:S's, HL2:DM's and TF2's). Halo's
//! (`halo_ce`): the disc image's files through its mount, with the build
//! its maps name (`halo_ce::game::mount::build`).

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use bevy::math::{Quat, Vec3};
use halo_ce::game as halo;
use hl2dm::game as hl2dm;
use tf2::game as tf2;
use dods::game as dods;
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
  games: cs_source, combat_arms, hl2dm, tf2, dods, halo_ce
  (no option)          summary: file counts and sizes by extension (hl2dm, tf2, dods: the
                       search path from gameinfo.txt first, each entry and whether it's there)
  --list               list every file with its size
  --archives           combat_arms: per-archive title, entropy and file count
  --sequences <model>  cs_source, hl2dm, tf2: a model's bones, sequences (activity, frames, fps,
                       duration, looping, ground speed) and pose parameters, includes merged
  --lods <model|text>  cs_source: each model's LODs (switch point, triangles) from its .dx90.vtx;
                       with text, every .mdl whose path contains it
  --objects <map>      combat_arms: a world file's objects (class, every property) and
                       its world models' boxes; --filter keeps objects whose class,
                       name or a property value contains <text>
  --script <name>      tf2: a script (`tf_weapon_scattergun`, `playerclasses/scout`) as text on
                       stdout only (decrypted with the local key when encrypted; the source on stderr)
  --gun-numbers        tf2: each gun's numbers' source (its script, or the spec fallback) and what the
                       script set (specs/tf2/weapon_scripts.md), through the game's mount (store/install)
  --ice-probe          tf2: which ICE level (thin, ice) decrypts scripts/tf_weapon_rocketlauncher.ctx
                       with the local key (CHIMERA_TF2_CTX_KEY or [games.tf2.keys.ctx]), and how many
                       of the install's .ctx scripts decrypt at the configured level
  --loops              every .wav (filtered) whose own cue or smpl chunk makes it loop
                       (specs/source/sound_looping.md): its loop start and length; then counts
  --filter <text>      only paths containing <text> (case-insensitive)
  --extract            write the (filtered) files to --out
  --out <dir>          extraction folder (default: per-user data dir; never inside the repo)
  --install <dir>      install folder (default: from chimera.local.toml)";

struct Args {
    game: String,
    list: bool,
    loops: bool,
    archives: bool,
    extract: bool,
    filter: Option<String>,
    sequences: Option<String>,
    lods: Option<String>,
    objects: Option<String>,
    script: Option<String>,
    ice_probe: bool,
    gun_numbers: bool,
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
        loops: false,
        archives: false,
        extract: false,
        filter: None,
        sequences: None,
        lods: None,
        objects: None,
        script: None,
        ice_probe: false,
        gun_numbers: false,
        out: None,
        install: None,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag}: missing value"));
        match flag.as_str() {
            "--list" => a.list = true,
            "--loops" => a.loops = true,
            "--archives" => a.archives = true,
            "--extract" => a.extract = true,
            "--filter" => a.filter = Some(value()?.to_lowercase()),
            "--sequences" => a.sequences = Some(value()?.to_lowercase().replace('\\', "/")),
            "--lods" => a.lods = Some(value()?.to_lowercase().replace('\\', "/")),
            "--objects" => a.objects = Some(value()?.to_lowercase()),
            "--script" => a.script = Some(value()?.to_lowercase().replace('\\', "/")),
            "--ice-probe" => a.ice_probe = true,
            "--gun-numbers" => a.gun_numbers = true,
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
        tf2::GAME => tf2::INFO.steam,
        dods::GAME => dods::INFO.steam,
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
        tf2::GAME => dump_tf2(args, &install),
        dods::GAME => dump_dods(args, &install),
        halo::GAME => dump_halo(args, &install),
        other => Err(format!("unknown game `{other}`; expected cs_source, combat_arms, hl2dm, tf2, dods or halo_ce")),
    }
}

/// DoD:S: the search path its `gameinfo.txt` gives, then the mount's
/// summary (or a model's sequences).
fn dump_dods(args: &Args, install: &Path) -> Result<(), String> {
    if let Ok(path) = dods::mount::search_path(install) {
        println!("gameinfo.txt search path (first wins):");
        for layer in &path.layers {
            println!("  {layer}");
        }
    }
    let mount = dods::mount::open(install).map_err(|e| e.to_string())?;
    if let Some(path) = &args.sequences {
        return sequences(&mount, path);
    }
    report(args, &mount, &[])
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
    if let Some(path) = &args.sequences {
        return sequences(&mount, path);
    }
    report(args, &mount, &[])
}

/// Halo: the disc image's (or folder's) build, then the mount's summary.
fn dump_halo(args: &Args, install: &Path) -> Result<(), String> {
    if let Ok(build) = halo::mount::build(install) {
        println!("build {build} (its maps' headers)");
    }
    let mount = halo::mount::open(install).map_err(|e| e.to_string())?;
    report(args, &mount, &[])
}

/// TF2: the search path its `tf/gameinfo.txt` gives (when the install's
/// own folders are there), then the mount's summary. An archive a `_dir`
/// file lists but that is missing (a dedicated server's textures and
/// sounds) leaves its files out of the mount.
fn dump_tf2(args: &Args, install: &Path) -> Result<(), String> {
    if let Some(name) = &args.script {
        return tf2_script(install, name);
    }
    if args.ice_probe {
        return tf2_ice_probe(install);
    }
    if args.gun_numbers {
        let mount = tf2::mount::open(install).map_err(|e| e.to_string())?;
        let key = tf2::scripts::key()?;
        let numbers = tf2::weapons::script_numbers::GunNumbers::from_mount(Some(&mount), key.as_ref());
        for s in &numbers.sources {
            println!("{}: {}", s.gun, s.source);
        }
        numbers.parse().map_err(|e| format!("the mapped documents don't load: {e}"))?;
        println!("the mapped documents load as weapon data");
        println!("differing from the committed data: {:?}", numbers.differences()?);
        return Ok(());
    }
    if let Ok(path) = tf2::mount::search_path(install) {
        println!("gameinfo.txt search path (first wins):");
        for layer in &path.layers {
            println!("  {layer}");
        }
    }
    let mount = tf2::mount::open(install).map_err(|e| e.to_string())?;
    if let Some(path) = &args.sequences {
        return sequences(&mount, path);
    }
    report(args, &mount, &[])
}

/// `--script`: the script's text on stdout alone (never into a file), its
/// source on stderr.
fn tf2_script(install: &Path, name: &str) -> Result<(), String> {
    let mount = tf2::mount::open(install).map_err(|e| e.to_string())?;
    let path = tf2::scripts::path_of(name.trim_end_matches(".ctx").trim_start_matches("scripts/"));
    let bytes = mount.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let key = tf2::scripts::key()?;
    let cipher = tf2::ice::CtxCipher::configured();
    let text = if tf2::scripts::parses(&bytes) {
        eprintln!("{path}: plain");
        bytes
    } else {
        let key = key.ok_or_else(|| format!("{path}: encrypted, and no key ({} or [games.tf2.keys.ctx])", tf2::scripts::KEY_ENV))?;
        let plain = tf2::ice::decrypt_ctx(&cipher.ice(&key.0), &bytes);
        if !tf2::scripts::parses(&plain) {
            return Err(format!("{path}: cipher {} doesn't decrypt it with the key; run `dump tf2 --ice-probe`", cipher.name()));
        }
        eprintln!("{path}: decrypted with the local key, cipher {}", cipher.name());
        plain
    };
    use std::io::Write;
    std::io::stdout().write_all(tf2::scripts::text_of(&text).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

/// `--ice-probe`: which levels decrypt the Rocket Launcher's script with
/// the local key, then how many of the install's `.ctx` scripts decrypt
/// to KeyValues at the configured level. Prints names and counts only,
/// never a script's text.
fn tf2_ice_probe(install: &Path) -> Result<(), String> {
    let mount = tf2::mount::open(install).map_err(|e| e.to_string())?;
    let path = "scripts/tf_weapon_rocketlauncher.ctx";
    let bytes = mount.read(path).map_err(|e| format!("{path}: {e}"))?;
    if tf2::scripts::parses(&bytes) {
        return Err(format!("{path} is plain here (the asset store's record): probe an install's encrypted file"));
    }
    let key = tf2::scripts::key()?.ok_or_else(|| format!("no key: set {} or [games.tf2.keys.ctx] key", tf2::scripts::KEY_ENV))?;
    let found = tf2::ice::probe_ctx(&key.0, &bytes);
    if found.is_empty() {
        return Err(format!("no ICE level decrypts {path} with this key: the key is wrong"));
    }
    for c in &found {
        println!("{path}: decrypts at level {}", c.name());
    }
    let cipher = tf2::ice::CtxCipher::configured();
    let ice = cipher.ice(&key.0);
    let (mut ok, mut failed) = (0, Vec::new());
    for (e, _) in mount.entries() {
        if !(e.path.starts_with("scripts/") && e.path.ends_with(".ctx")) {
            continue;
        }
        let bytes = mount.read(&e.path).map_err(|err| format!("{}: {err}", e.path))?;
        if tf2::scripts::parses(&tf2::ice::decrypt_ctx(&ice, &bytes)) {
            ok += 1;
        } else {
            failed.push(e.path.clone());
        }
    }
    println!("level {}: {ok} .ctx scripts decrypt, {} don't {failed:?}", cipher.name(), failed.len());
    if !found.contains(&cipher) {
        eprintln!("set {}=<level> to use another level (games/tf2/src/game/ice.rs)", tf2::ice::LEVEL_ENV);
    }
    Ok(())
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

    if args.loops {
        loops(mount, &entries);
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

/// Each WAV that loops by its own chunks (`--loops`): loop start, length.
fn loops(mount: &Mount, entries: &[(chimera_core::mount::Entry, usize)]) {
    let (mut wavs, mut looping, mut failed) = (0, 0, 0);
    for (e, _) in entries.iter().filter(|(e, _)| e.path.ends_with(".wav")) {
        wavs += 1;
        let Some(clip) = mount.read(&e.path).ok().and_then(|b| chimera_world::map::sound::decode_clip(&b).ok()) else {
            failed += 1;
            continue;
        };
        let frames = clip.samples.len() / clip.channels.max(1) as usize;
        if let Some(start) = clip.loop_start.filter(|s| *s < frames) {
            looping += 1;
            let rate = clip.rate.max(1) as f64;
            println!("loop {:>9} of {:>9} frames ({:.3} s of {:.3} s)  {}", start, frames, start as f64 / rate, frames as f64 / rate, e.path);
        }
    }
    println!("{wavs} wav files, {looping} loop by their own chunks, {failed} unreadable");
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
