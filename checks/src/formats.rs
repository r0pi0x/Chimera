//! The generated table of every converted format and its version
//! (`docs/formats.md`, docs/plans/active/converted-cache.md, "Converter
//! versions"), written by `docs_index` with the module list: the formats
//! numbered by hand (the asset manifest's `FORMAT`, the converted-map
//! cache entry's `MAGIC`, the served recipe's `RECIPE_FORMAT`, the map
//! format's `FORMAT_BUMP`, Halo's character units' `VERSION`) with their
//! numbers, and each converter whose version a build script makes (every
//! `build.rs` that sets `CHIMERA_CONVERTER_SOURCES` or
//! `CHIMERA_MAP_SOURCES`) with its hand-bumped number, the paths it hashes
//! and the parsing crates whose versions it takes. The hashes themselves
//! change with each edit of those paths and aren't listed (a running
//! build prints them: `chimera_pack convert --list`); a number or a hashed
//! path changed without the table fails `generated_docs_are_fresh`.

use std::{fs, path::Path};

/// `const <name>: ... = <value>;` in `code` (the value as written).
fn constant<'a>(code: &'a str, name: &str) -> Option<&'a str> {
    let at = code.find(&format!("const {name}: "))?;
    let rest = &code[at..];
    let value = &rest[rest.find(" = ")? + 3..];
    Some(value[..value.find(";\n")?].trim())
}

/// The string literals of the array that starts after `start` in `code`
/// (to its `]`), comments skipped.
fn strings_after(code: &str, start: &str) -> Vec<String> {
    let Some(at) = code.find(start) else { return Vec::new() };
    let rest = &code[at + start.len()..];
    let end = rest.find("];").or_else(|| rest.find(']')).unwrap_or(rest.len());
    rest[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .flat_map(|l| l.split('"').skip(1).step_by(2).map(str::to_string).collect::<Vec<_>>())
        .collect()
}

fn build_scripts(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for top in ["chimera", "games", "source", "lithtech", "blam", "goldsrc", "apps"] {
        let Ok(rd) = fs::read_dir(root.join(top)) else { continue };
        for e in rd.flatten() {
            let b = e.path().join("build.rs");
            if b.is_file() {
                out.push(format!("{top}/{}/build.rs", e.file_name().to_string_lossy()));
            }
        }
    }
    out.sort();
    out
}

fn code(root: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(root.join(path))
        .map(|c| c.replace("\r\n", "\n"))
        .map_err(|e| format!("{path}: {e}"))
}

/// `docs/formats.md` as the sources say.
pub fn generate(root: &Path) -> Result<String, String> {
    let mut out = String::from(
        "# Converted formats and their versions\n\n\
         Generated from the sources by `cargo run -q -p repo_checks --bin docs_index`\n\
         (`repo_checks::formats`): do not edit. The architecture test\n\
         `generated_docs_are_fresh` fails when a number or a hashed path below\n\
         changes without this file. The policy (when to bump a number, what a\n\
         converter hashes, how long the asset service keeps a version):\n\
         [plans/active/converted-cache.md](plans/active/converted-cache.md),\n\
         \"Converter versions\" and \"Served entries\".\n\n\
         ## Numbered by hand\n\n\
         | Format | Version | Where |\n|---|---|---|\n",
    );
    let numbered: [(&str, &str, &str); 6] = [
        ("Asset manifest (`chimera-manifest`)", "chimera/assets/src/manifest.rs", "FORMAT"),
        ("Converted-map cache entry (`CHIMAP`, also a served entry)", "chimera/assets/src/map_cache.rs", "MAGIC"),
        ("Served entry's recipe (`chimera-recipe`)", "chimera/assets/src/map_cache/served.rs", "RECIPE_FORMAT"),
        ("Map format's bump (with the hash of its sources, below)", "chimera/world/src/map/cache.rs", "FORMAT_BUMP"),
        ("Halo's campaign character units (`halo_ce:character_unit`)", "games/halo_ce/src/game/campaign/unit.rs", "VERSION"),
        ("TF2's item pack (`tf2:item_stats`)", "games/tf2/src/game/items/mod.rs", "VERSION"),
    ];
    for (what, path, name) in numbered {
        let c = code(root, path)?;
        let value = constant(&c, name).ok_or_else(|| format!("{path}: no const {name}"))?;
        let value = if name == "MAGIC" {
            // b"CHIMAP\0\x03": the last byte is the version.
            let hex = value.rsplit("\\x").next().unwrap_or("").trim_end_matches('"');
            u8::from_str_radix(hex, 16).map(|v| v.to_string()).map_err(|_| format!("{path}: MAGIC's version"))?
        } else {
            value.to_string()
        };
        out.push_str(&format!("| {what} | {value} | `{path}` `{name}` |\n"));
    }
    out.push_str(
        "\n## Made by build scripts\n\n\
         A converter's version is `v<number>-<hash>`: the number bumped by hand and\n\
         the hash of the paths it lists and of the parsing crates' versions in\n\
         `Cargo.lock`. The map format's is the hash of its paths and `FORMAT_BUMP`.\n\n\
         | Build script | Variable | Number | Hashed paths | Parsing crates |\n|---|---|---|---|---|\n",
    );
    for script in build_scripts(root) {
        let c = code(root, &script)?;
        let var = if c.contains("CHIMERA_CONVERTER_SOURCES") {
            "CHIMERA_CONVERTER_SOURCES"
        } else if c.contains("CHIMERA_MAP_SOURCES") {
            "CHIMERA_MAP_SOURCES"
        } else {
            continue;
        };
        let number = constant(&c, "VERSION").unwrap_or("-");
        let mut paths = strings_after(&c, "const CONVERTING: &[&str] = &[");
        if paths.is_empty() {
            paths = strings_after(&c, "let paths = [");
        }
        let crates = strings_after(&c, "const PARSERS: &[&str] = &[");
        let list = |v: &[String]| if v.is_empty() { "-".to_string() } else { v.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ") };
        out.push_str(&format!("| `{script}` | `{var}` | {number} | {} | {} |\n", list(&paths), list(&crates)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_arrays_are_read() {
        let c = "const VERSION: u32 = 3;\nconst CONVERTING: &[&str] = &[\n    // a note \"x\"\n    \"src/a.rs\",\n    \"data\",\n];\n";
        assert_eq!(constant(c, "VERSION"), Some("3"));
        assert_eq!(strings_after(c, "const CONVERTING: &[&str] = &["), ["src/a.rs", "data"]);
    }
}
