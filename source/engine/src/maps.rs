//! A Source game's maps as its install lists them: `maps/<name>.bsp`
//! (`map_names`, a game's `GameInfo::map_names`) and the map cycle a
//! server plays in turn (`map_cycle`, `GameInfo::map_cycle`), read from
//! the game's cycle files; maps copied into Chimera's content cache
//! (`import_map_into`, a game's `GameInfo::import_map`). Lifted from `cs_source::game` for HL2:DM
//! (docs/plans/active/hl2dm.md, M0); CS:S re-exports them at their old
//! paths.

use std::{collections::BTreeSet, path::Path};

use chimera_core::mount::Mount;

/// Every map the mounted install holds: `maps/<name>.bsp`, sorted.
pub fn map_names(mount: &Mount) -> Vec<String> {
    let mut names: Vec<String> = mount
        .entries()
        .into_iter()
        .filter_map(|(e, _)| {
            e.path
                .strip_prefix("maps/")
                .and_then(|p| p.strip_suffix(".bsp"))
                .filter(|p| !p.contains('/'))
                .map(String::from)
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The map cycle a server plays in turn (`cfg/mapcycle.txt`, else the
/// install's `cfg/mapcycle_default.txt`, else an old `mapcycle.txt`): its
/// map names in order, those the install has.
pub fn map_cycle(mount: &Mount) -> Vec<String> {
    let maps: BTreeSet<String> = mount
        .entries()
        .into_iter()
        .filter_map(|(e, _)| {
            e.path
                .strip_prefix("maps/")?
                .strip_suffix(".bsp")
                .map(str::to_lowercase)
        })
        .collect();
    let has_map = |m: &String| maps.contains(m);
    ["cfg/mapcycle.txt", "cfg/mapcycle_default.txt", "mapcycle.txt"]
        .iter()
        .find_map(|p| mount.read(p).ok())
        .map(|b| parse_map_cycle(&String::from_utf8_lossy(&b)))
        .unwrap_or_default()
        .into_iter()
        .filter(has_map)
        .collect()
}

/// A map cycle file's map names: `//` comments and blank lines left out.
pub fn parse_map_cycle(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split("//").next().unwrap_or_default().trim())
        .filter_map(|l| l.split_whitespace().next().map(str::to_lowercase))
        .collect()
}

/// Copy a map (`.bsp`, or `.bsp.bz2` as servers send them) into the
/// content folder `cache` (a game's, `mount::config::content_dir`) as
/// `maps/<name>.bsp`, and note it in `index.toml` (size, FNV-1a 64 hash,
/// where it came from). Returns the map's name.
pub fn import_map_into(file: &Path, cache: &Path) -> Result<String, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let (bsp, name) = if let Some(stem) = file_name.strip_suffix(".bsp.bz2") {
        let mut out = Vec::new();
        use std::io::Read;
        bzip2::read::BzDecoder::new(&bytes[..])
            .read_to_end(&mut out)
            .map_err(|e| format!("{}: not a bzip2 file ({e})", file.display()))?;
        (out, stem.to_string())
    } else if let Some(stem) = file_name.strip_suffix(".bsp") {
        (bytes, stem.to_string())
    } else {
        return Err(format!("{}: expected a .bsp or .bsp.bz2 file", file.display()));
    };
    if bsp.get(0..4) != Some(b"VBSP") {
        return Err(format!("{}: not a Source map (no VBSP header)", file.display()));
    }
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(format!("{}: bad map name", file.display()));
    }
    let maps = cache.join("maps");
    std::fs::create_dir_all(&maps).map_err(|e| format!("{}: {e}", maps.display()))?;
    let target = maps.join(format!("{name}.bsp"));
    std::fs::write(&target, &bsp).map_err(|e| format!("{}: {e}", target.display()))?;
    let hash = bsp.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    });
    let entry = format!(
        "[[map]]\nname = {name:?}\nsize = {}\nfnv64 = \"{hash:016x}\"\nfrom = {:?}\n\n",
        bsp.len(),
        file.display().to_string()
    );
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache.join("index.toml"))
        .and_then(|mut f| f.write_all(entry.as_bytes()))
        .map_err(|e| format!("index.toml: {e}"))?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_cycle_files_read_as_names() {
        let text = "// Default mapcycle file\r\n//\r\n\r\ncs_italy\r\nde_dust // comment\n De_Aztec \n";
        assert_eq!(super::parse_map_cycle(text), ["cs_italy", "de_dust", "de_aztec"]);
    }
}
