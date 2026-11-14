//! Which packages' tests a change can affect (the `affected` binary; the
//! git hooks, .githooks/pre-commit and pre-push, run only those).
//!
//! Each changed file is classified (`classify`):
//! - everything (`Change::All`): the workspace manifest, the test
//!   runner's and hakari's settings (.config/), the hooks, any `build.rs`,
//!   the workspace-hack, this crate's selection code, and any path no rule
//!   below knows (conservative); the lockfile too when its two versions
//!   aren't given (`Selection::with_lock`: with them, the members whose
//!   resolved dependencies changed, `lock_affected`, as if their code
//!   had);
//! - documentation (`Change::Docs`): Markdown anywhere, licences, docs/,
//!   specs/, scripts/, dist/, the analysis, probe and setup scripts under tools/
//!   and .claude/settings.json: only
//!   this crate's checks (CLAUDE.md's links, one-line docs entries, the
//!   spec rule);
//! - a package (`Change::Package`): a file of a workspace member outside
//!   its `tests/` (its sources, manifest, shaders, the data files its tools
//!   read under the root's tools/): its tests and the tests of every
//!   package that depends on it, as a dependency or a dev-dependency, at
//!   any depth (`Workspace::test_deps`);
//! - a feature's part (`Change::Feature`): a file under a folder only a
//!   feature compiles, as its manifest says (`[package.metadata.affected]
//!   feature-paths = { ui = ["src/ui/"] }`: cs_source's front end): the
//!   package's test binaries built with that feature, its unit tests of
//!   that module (`binary_id(cs_source) & test(/^ui::/)`) and the tests
//!   of every package depending on it, not its other binaries (the
//!   simulation's scenario and heavy map tests);
//! - test code (`Change::Tests`): a file under a member's `tests/`: only
//!   that package's tests, nothing above it can see them. A test module
//!   (`apps/cs_source/tests/it/heavy/map_surf.rs`) selects that module's tests
//!   (`heavy::map_surf::`) plus the modules of the same binary that name it
//!   (`super::map_surf::`); the binary's `main.rs`, a `mod.rs`, a deleted
//!   file or a file of no binary (a shared helper such as
//!   `tests/common/mod.rs`) select all of the package's test binaries.
//!
//! The graph is `cargo metadata --all-features`: optional dependencies
//! count (a game's `ui` front end's), so it never misses one a feature
//! turns on. This crate's own tests (the architecture tests) always run:
//! any change can break a layering or repository rule.
//!
//! Known shortcuts: the selection is per package (or test module, or a
//! feature's folder), not per test binary's features beyond
//! `feature-paths`: `cs_source`'s simulation tests run for a change to
//! the presentation its `ui` feature uses (`chimera_present` is under
//! both). The scenario tests of a crate run for any change below it:
//! most of them (the heavy map tests above all) are `cs_source`'s, so an
//! edit to Chimera or Source's logic still selects nearly the whole suite.
//! A test that reads a repository file at run time (not through
//! `include_str!`) outside its package isn't known; none does today
//! (`tools/testmap` is mapped to `source_tools`, the tools' data to
//! `cs_source_tools`, the minigames' definitions, `minigames/`, to
//! `originals`).

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// This crate: its tests run on every selection.
pub const CHECKS: &str = "repo_checks";

/// `Selection::partial`'s name for a package's unit tests (its library's
/// test binary, nextest's `binary_id(<package>)`).
pub const LIB: &str = "(unit tests)";

/// A workspace member as the selection needs it.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    /// Its folder relative to the workspace root, `/`-separated; empty
    /// for the root package.
    pub dir: String,
    /// Workspace members its library and binaries use (normal and build
    /// dependencies).
    pub deps: Vec<String>,
    /// Workspace members only its tests use (dev-dependencies).
    pub dev_deps: Vec<String>,
    /// Its integration test binaries.
    pub tests: Vec<TestBinary>,
    /// Folders only a feature compiles (`[package.metadata.affected]
    /// feature-paths` in its manifest): (path prefix inside the package,
    /// e.g. `src/ui/`, and the feature).
    pub feature_paths: Vec<(String, String)>,
}

/// An integration test binary (`[[test]]`, or one cargo found in `tests/`).
#[derive(Clone, Debug)]
pub struct TestBinary {
    pub name: String,
    /// Its crate root (`tests/it/main.rs`) relative to the workspace root.
    pub main: String,
    /// Features it needs (`required-features`): turned on when it's
    /// selected.
    pub required_features: Vec<String>,
}

impl TestBinary {
    /// The folder its modules live in (`tests/it` for `tests/it/main.rs`);
    /// None for a single-file binary (`tests/foo.rs`).
    fn module_dir(&self) -> Option<&str> {
        self.main.strip_suffix("/main.rs")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Workspace {
    pub packages: Vec<Package>,
}

impl Workspace {
    /// The workspace at `root`, from `cargo metadata --all-features`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let run = |offline: bool| {
            let mut cmd = Command::new(&cargo);
            cmd.current_dir(root)
                .args(["metadata", "--format-version", "1", "--all-features"]);
            if offline {
                cmd.arg("--offline");
            }
            cmd.output()
        };
        let out = match run(true) {
            Ok(o) if o.status.success() => o,
            _ => run(false).map_err(|e| format!("cargo metadata: {e}"))?,
        };
        if !out.status.success() {
            return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
        }
        let json: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata's output: {e}"))?;
        Self::from_metadata(&json)
    }

    /// The workspace from `cargo metadata --format-version 1` output.
    pub fn from_metadata(json: &serde_json::Value) -> Result<Self, String> {
        let root = json["workspace_root"].as_str().ok_or("no workspace_root")?;
        let root = Path::new(root);
        let members: BTreeSet<&str> = json["workspace_members"]
            .as_array()
            .ok_or("no workspace_members")?
            .iter()
            .filter_map(|m| m.as_str())
            .collect();
        let mut names = BTreeMap::new();
        let mut packages = BTreeMap::new();
        for p in json["packages"].as_array().ok_or("no packages")? {
            let id = p["id"].as_str().unwrap_or_default();
            if !members.contains(id) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or_default().to_string();
            let manifest = Path::new(p["manifest_path"].as_str().unwrap_or_default());
            let dir = rel(root, manifest.parent().unwrap_or(root));
            let mut tests = Vec::new();
            for t in p["targets"].as_array().into_iter().flatten() {
                let kinds: Vec<&str> = t["kind"].as_array().into_iter().flatten().filter_map(|k| k.as_str()).collect();
                if !kinds.contains(&"test") {
                    continue;
                }
                tests.push(TestBinary {
                    name: t["name"].as_str().unwrap_or_default().to_string(),
                    main: rel(root, Path::new(t["src_path"].as_str().unwrap_or_default())),
                    required_features: t["required-features"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|f| f.as_str().map(String::from))
                        .collect(),
                });
            }
            // `feature-paths = { ui = ["src/ui/"] }`.
            let mut feature_paths = Vec::new();
            for (feature, paths) in p["metadata"]["affected"]["feature-paths"].as_object().into_iter().flatten() {
                for path in paths.as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                    feature_paths.push((path.to_string(), feature.clone()));
                }
            }
            names.insert(id.to_string(), name.clone());
            packages.insert(
                id.to_string(),
                Package {
                    name,
                    dir,
                    deps: Vec::new(),
                    dev_deps: Vec::new(),
                    tests,
                    feature_paths,
                },
            );
        }
        for node in json["resolve"]["nodes"].as_array().ok_or("no resolve graph")? {
            let id = node["id"].as_str().unwrap_or_default();
            let Some(package) = packages.get(id) else { continue };
            let mut package = package.clone();
            for dep in node["deps"].as_array().into_iter().flatten() {
                let Some(name) = dep["pkg"].as_str().and_then(|d| names.get(d)) else {
                    continue;
                };
                for kind in dep["dep_kinds"].as_array().into_iter().flatten() {
                    let list = match kind["kind"].as_str() {
                        Some("dev") => &mut package.dev_deps,
                        _ => &mut package.deps,
                    };
                    if !list.contains(name) {
                        list.push(name.clone());
                    }
                }
            }
            packages.insert(id.to_string(), package);
        }
        Ok(Self {
            packages: packages.into_values().collect(),
        })
    }

    pub fn package(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// The members `name`'s library builds on, itself included: its
    /// normal and build dependencies, at any depth.
    fn lib_deps(&self, name: &str, out: &mut BTreeSet<String>) {
        if !out.insert(name.to_string()) {
            return;
        }
        if let Some(p) = self.package(name) {
            for d in &p.deps {
                self.lib_deps(d, out);
            }
        }
    }

    /// The members `name`'s tests build on, itself included: its library's
    /// (`lib_deps`) and its dev-dependencies' libraries'.
    pub fn test_deps(&self, name: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.lib_deps(name, &mut out);
        if let Some(p) = self.package(name) {
            for d in &p.dev_deps {
                self.lib_deps(d, &mut out);
            }
        }
        out
    }

    /// The member whose folder holds `path` (the deepest one), with the
    /// rest of the path. Files at the root belong to the root package only
    /// in its own folders (`src/`, `tests/`, its `Cargo.toml` is the
    /// workspace's).
    fn owner<'a>(&self, path: &'a str) -> Option<(&Package, &'a str)> {
        let mut best: Option<(&Package, &str)> = None;
        for p in &self.packages {
            let rest = if p.dir.is_empty() {
                if !(path.starts_with("src/") || path.starts_with("tests/")) {
                    continue;
                }
                Some(path)
            } else {
                path.strip_prefix(&p.dir).and_then(|r| r.strip_prefix('/'))
            };
            if let Some(rest) = rest
                && best.is_none_or(|(b, _)| p.dir.len() > b.dir.len())
            {
                best = Some((p, rest));
            }
        }
        best
    }
}

/// `path` relative to `root`, `/`-separated.
fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// What a changed file can affect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Anything: run every test (why).
    All(String),
    /// Documentation: this crate's checks only.
    Docs,
    /// A package's library or binaries: its tests and those of every
    /// package depending on it.
    Package(String),
    /// A part of a package's library only `feature` compiles (`module`
    /// of it, a `::` path): the package's test binaries built with the
    /// feature, its unit tests of that module, and every package
    /// depending on it.
    Feature {
        package: String,
        feature: String,
        module: String,
    },
    /// A package's test code: its test `binary` (all of them if None),
    /// `module` of it if Some (a `::` path, e.g. `heavy::map_surf`).
    Tests {
        package: String,
        binary: Option<String>,
        module: Option<String>,
    },
}

/// Files that change what every build or test run does.
fn changes_everything(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    Some(match path {
        "Cargo.lock" => "the lockfile",
        "Cargo.toml" => "the workspace manifest",
        "rust-toolchain" | "rust-toolchain.toml" => "the toolchain",
        ".gitattributes" => "the files' attributes",
        _ if path.starts_with(".config/") => "the test runner's or hakari's settings (.config/)",
        _ if path.starts_with(".cargo/") => "cargo's settings (.cargo/)",
        _ if path.starts_with(".githooks/") => "the git hooks",
        _ if path.starts_with("chimera/workspace-hack/") => "the workspace-hack",
        _ if path.starts_with("checks/src/") || path == "checks/Cargo.toml" => "the test selection itself",
        _ if name == "build.rs" => "a build script",
        _ => return None,
    })
}

/// The workspace-hack (hakari's, chimera/workspace-hack): every member
/// depends on it and it on every unified third-party crate, so the
/// lockfile's graph is walked around it (`lock_affected`); its own
/// changes select everything (`changes_everything`).
pub const WORKSPACE_HACK: &str = "workspace-hack";

/// One `[[package]]` of a Cargo.lock: its key (`name version source`),
/// checksum and dependencies (as the lockfile names them).
#[derive(Clone, Debug, PartialEq, Eq)]
struct LockPackage {
    name: String,
    version: String,
    source: String,
    checksum: String,
    deps: Vec<String>,
}

/// A parsed Cargo.lock: packages by key, and their keys by name.
struct Lock(BTreeMap<String, LockPackage>, BTreeMap<String, Vec<String>>);

impl Lock {
    fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Table = text.parse().map_err(|e| format!("Cargo.lock: {e}"))?;
        let mut out = BTreeMap::new();
        for p in value.get("package").and_then(|p| p.as_array()).into_iter().flatten() {
            let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let pkg = LockPackage {
                name: s("name"),
                version: s("version"),
                source: s("source"),
                checksum: s("checksum"),
                deps: p
                    .get("dependencies")
                    .and_then(|d| d.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|d| d.as_str().map(String::from))
                    .collect(),
            };
            out.insert(format!("{} {} {}", pkg.name, pkg.version, pkg.source), pkg);
        }
        let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, p) in &out {
            by_name.entry(p.name.clone()).or_default().push(k.clone());
        }
        Ok(Self(out, by_name))
    }

    /// The key a dependency entry names: `name`, `name version` or `name
    /// version (source)`.
    fn resolve(&self, dep: &str) -> Option<&str> {
        let mut parts = dep.splitn(3, ' ');
        let name = parts.next()?;
        let version = parts.next();
        let source = parts.next().map(|s| s.trim_start_matches('(').trim_end_matches(')'));
        self.1.get(name)?.iter().map(String::as_str).find(|k| {
            let p = &self.0[*k];
            version.is_none_or(|v| p.version == v) && source.is_none_or(|s| p.source == s)
        })
    }

    /// The dependencies as keys (unresolvable entries left out).
    fn deps_of(&self, key: &str) -> BTreeSet<String> {
        self.0[key].deps.iter().filter_map(|d| self.resolve(d)).map(String::from).collect()
    }

    /// Every key `from` reaches, itself included, not through the
    /// workspace-hack.
    fn closure(&self, from: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut todo = vec![from.to_string()];
        while let Some(k) = todo.pop() {
            if !out.insert(k.clone()) {
                continue;
            }
            todo.extend(self.deps_of(&k).into_iter().filter(|d| self.0[d].name != WORKSPACE_HACK));
        }
        out
    }
}

/// What a Cargo.lock change can affect: each workspace member (by name)
/// whose resolved dependency graph, before or after, holds a package that
/// was added, removed, or changed checksum or dependencies, with those
/// packages (`name version`). The graph is the lockfile's, a superset of
/// any build's (every target, feature and platform), walked around the
/// workspace-hack (hakari's feature unification: a change to it is its
/// manifest's, which selects everything). Members only (no `source`).
pub fn lock_affected(old: &str, new: &str) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let (old, new) = (Lock::parse(old)?, Lock::parse(new)?);
    let mut changed = BTreeSet::new();
    for (k, p) in &new.0 {
        match old.0.get(k) {
            Some(o) if o.checksum == p.checksum && old.deps_of(k) == new.deps_of(k) => {}
            _ => {
                changed.insert(k.clone());
            }
        }
    }
    changed.extend(old.0.keys().filter(|k| !new.0.contains_key(*k)).cloned());
    let label = |k: &str| {
        let mut it = k.split(' ');
        format!("{} {}", it.next().unwrap_or(k), it.next().unwrap_or_default())
    };
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for lock in [&old, &new] {
        for (k, p) in &lock.0 {
            if !p.source.is_empty() || p.name == WORKSPACE_HACK {
                continue;
            }
            let hit: BTreeSet<String> = lock.closure(k).intersection(&changed).map(|c| label(c)).collect();
            if !hit.is_empty() {
                out.entry(p.name.clone()).or_default().extend(hit);
            }
        }
    }
    Ok(out)
}

/// Files no test reads: documentation and scripts for people.
fn is_docs(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.ends_with(".md")
        || name.starts_with("LICENSE")
        || [
            "docs/",
            "specs/",
            "scripts/",
            "dist/",
            "tools/arch_survey/",
            "tools/css_probe/",
            "tools/hl2dm_probe/",
            "tools/renderdoc/",
            // The UI parity check's scripts and look book pages (no crate reads them).
            "tools/ui-parity/",
            // The UGG fonts' build scripts (their output is in apps/ultimate_gun_game/fonts/).
            "tools/fonts/",
            // The UGG UI sounds' generator (its output is in apps/ultimate_gun_game/assets/sounds/ui/).
            "tools/sounds/",
        ]
        .iter()
        .any(|p| path.starts_with(p))
        || [
            ".gitignore",
            "rustfmt.toml",
            "chimera.local.example.toml",
            // The agents' shell tools: no crate reads them.
            "tools/agent-env.sh",
            "tools/sccache-heal.sh",
            "tools/jobserver-heal.sh",
            "tools/clean-worktrees.sh",
            "tools/agent-slot.sh",
            "tools/disk-free.sh",
            "tools/post-push-tests.sh",
            // A profiling script (runs built binaries; no crate reads it).
            "tools/perf-profile.sh",
            // Cuts a frame log's worst frames out of a profile (no crate reads it).
            "tools/frame_cut.py",
            // A frame dump and its sound as an MP4 (no crate reads it).
            "tools/dump_to_mp4.py",
            // A clone's setup and readiness checks (AGENTS.md, "Setup").
            "tools/setup",
            "tools/doctor",
            // Claude Code's project permissions (AGENTS.md, "Hard rules").
            ".claude/settings.json",
        ]
        .contains(&path)
}

/// The root's data files outside a crate (tools/'s, the minigames'
/// definitions) and the package that reads them.
const TOOL_DATA: &[(&str, &str)] = &[
    ("minigames/", "originals"),
    ("tools/testmap/", "source_tools"),
    ("tools/perfgate/", "cs_source_tools"),
    ("tools/refcmp/", "cs_source_tools"),
];

/// What changing `path` (relative to the root, `/`-separated) can affect.
/// `exists`: whether the file is still there (a deleted test module
/// selects its whole binary).
pub fn classify(ws: &Workspace, path: &str, exists: bool) -> Change {
    let path = path.trim_start_matches("./");
    if let Some(why) = changes_everything(path) {
        return Change::All(format!("{path}: {why}"));
    }
    if is_docs(path) {
        return Change::Docs;
    }
    if let Some((_, package)) = TOOL_DATA.iter().find(|(p, _)| path.starts_with(p)) {
        return Change::Package(package.to_string());
    }
    let Some((package, rest)) = ws.owner(path) else {
        return Change::All(format!("{path}: no rule for this path (add one to checks/src/affected.rs)"));
    };
    if !rest.starts_with("tests/") {
        if rest.starts_with("benches/") || rest.starts_with("examples/") {
            return Change::Tests {
                package: package.name.clone(),
                binary: None,
                module: None,
            };
        }
        if let Some((prefix, feature)) = package.feature_paths.iter().find(|(p, _)| rest.starts_with(p.as_str())) {
            let module = prefix.trim_start_matches("src/").trim_end_matches('/').replace('/', "::");
            return Change::Feature {
                package: package.name.clone(),
                feature: feature.clone(),
                module,
            };
        }
        return Change::Package(package.name.clone());
    }
    let whole = Change::Tests {
        package: package.name.clone(),
        binary: None,
        module: None,
    };
    // The binary this file is part of.
    for bin in &package.tests {
        if bin.main == path {
            return Change::Tests {
                package: package.name.clone(),
                binary: Some(bin.name.clone()),
                module: None,
            };
        }
        let Some(dir) = bin.module_dir() else { continue };
        let Some(file) = path.strip_prefix(dir).and_then(|r| r.strip_prefix('/')) else {
            continue;
        };
        let binary_only = Change::Tests {
            package: package.name.clone(),
            binary: Some(bin.name.clone()),
            module: None,
        };
        let Some(stem) = file.strip_suffix(".rs") else {
            return binary_only;
        };
        if !exists || stem.ends_with("mod") || stem.split('/').any(|s| s.is_empty()) {
            return binary_only;
        }
        return Change::Tests {
            package: package.name.clone(),
            binary: Some(bin.name.clone()),
            module: Some(stem.replace('/', "::")),
        };
    }
    whole
}

/// A test binary's modules to run (None: all of it), and why.
pub type Modules = (Option<BTreeSet<String>>, String);

/// The tests to run.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Everything, and why.
    pub all: Option<String>,
    /// Packages whose tests all run, and why.
    pub packages: BTreeMap<String, String>,
    /// Packages of which some test binaries run: binary -> its modules
    /// (None: all of it), and why.
    pub partial: BTreeMap<String, BTreeMap<String, Modules>>,
    /// Features the selected test binaries need (`package/feature`).
    pub features: BTreeSet<String>,
    /// Every changed file with what it affects, for the summary.
    pub files: Vec<(String, String)>,
    /// Packages with a changed file of their own (sources, a feature's
    /// part or test code; not the ones selected only for depending on
    /// them): what `direct` keeps.
    pub changed: BTreeSet<String>,
}

impl Selection {
    /// The tests `changed` (paths relative to `root`) can affect.
    pub fn new(ws: &Workspace, root: &Path, changed: &[String]) -> Self {
        Self::with_lock(ws, root, changed, None)
    }

    /// As `new`, with what the lockfile's change affects when known
    /// (`lock_affected` of its two versions, or why it couldn't be
    /// compared): only the members whose resolved dependencies changed,
    /// as if their own code had; without it, everything.
    pub fn with_lock(
        ws: &Workspace,
        root: &Path,
        changed: &[String],
        lock: Option<&Result<BTreeMap<String, BTreeSet<String>>, String>>,
    ) -> Self {
        let mut sel = Selection::default();
        let mut changed_packages: BTreeMap<String, String> = BTreeMap::new();
        let mut tests: Vec<(String, Option<String>, Option<String>, String)> = Vec::new();
        let mut feature_parts: Vec<(String, String, String, String)> = Vec::new();
        let mut everything = Vec::new();
        for path in changed {
            let change = match (path.as_str(), lock) {
                ("Cargo.lock", Some(Ok(members))) => {
                    let mut names = Vec::new();
                    for (member, crates) in members {
                        if ws.package(member).is_none() {
                            continue;
                        }
                        names.push(member.as_str());
                        let list: Vec<&str> = crates.iter().map(String::as_str).take(4).collect();
                        let more = crates.len().saturating_sub(list.len());
                        let more = if more > 0 { format!(" and {more} more") } else { String::new() };
                        let why = format!("Cargo.lock: {}{more} changed in what it builds on", list.join(", "));
                        changed_packages.entry(member.clone()).or_insert(why);
                    }
                    let what = if names.is_empty() {
                        "the lockfile: no member's resolved dependencies changed".to_string()
                    } else {
                        format!("the lockfile: packages whose resolved dependencies changed: {}", names.join(", "))
                    };
                    sel.files.push((path.clone(), what));
                    continue;
                }
                ("Cargo.lock", Some(Err(e))) => {
                    Change::All(format!("Cargo.lock: the lockfile (its versions couldn't be compared: {e})"))
                }
                _ => classify(ws, path, root.join(path).exists()),
            };
            let what = match &change {
                Change::All(why) => {
                    everything.push(why.clone());
                    "everything".to_string()
                }
                Change::Docs => "docs: the repository checks".to_string(),
                Change::Package(p) => {
                    changed_packages.entry(p.clone()).or_insert_with(|| path.clone());
                    format!("package {p}")
                }
                Change::Feature {
                    package,
                    feature,
                    module,
                } => {
                    feature_parts.push((package.clone(), feature.clone(), module.clone(), path.clone()));
                    format!("package {package}'s `{feature}` part ({module}::)")
                }
                Change::Tests { package, binary, module } => {
                    tests.push((package.clone(), binary.clone(), module.clone(), path.clone()));
                    match (binary, module) {
                        (Some(b), Some(m)) => format!("tests of {package}: {b} {m}::"),
                        (Some(b), None) => format!("tests of {package}: binary {b}"),
                        _ => format!("tests of {package}"),
                    }
                }
            };
            sel.files.push((path.clone(), what));
        }
        sel.changed = changed_packages.keys().cloned().collect();
        sel.changed.extend(feature_parts.iter().map(|f| f.0.clone()));
        sel.changed.extend(tests.iter().map(|t| t.0.clone()));
        if !everything.is_empty() {
            sel.all = Some(everything.join("; "));
            return sel;
        }
        if ws.package(CHECKS).is_some() {
            sel.packages
                .insert(CHECKS.to_string(), "always (the repository's rules)".to_string());
        }
        for p in &ws.packages {
            let deps = ws.test_deps(&p.name);
            let hit: Vec<&String> = changed_packages.keys().filter(|c| deps.contains(*c)).collect();
            let Some(first) = hit.first() else { continue };
            let why = if changed_packages.contains_key(&p.name) {
                format!("changed ({})", changed_packages[&p.name])
            } else if hit.len() == 1 {
                format!("its tests depend on {first} (changed)")
            } else {
                format!(
                    "its tests depend on {} (changed)",
                    hit.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                )
            };
            sel.packages.entry(p.name.clone()).or_insert(why);
        }
        // A feature's part: what depends on the package, and the package's
        // own tests built with the feature.
        for (package, feature, module, path) in &feature_parts {
            for p in &ws.packages {
                if p.name != *package && ws.test_deps(&p.name).contains(package) {
                    sel.packages
                        .entry(p.name.clone())
                        .or_insert_with(|| format!("its tests depend on {package} (changed: {path})"));
                }
            }
            if sel.packages.contains_key(package) {
                continue;
            }
            let Some(p) = ws.package(package) else { continue };
            let why = format!("its `{feature}` part changed ({path})");
            let bins = sel.partial.entry(package.clone()).or_default();
            for bin in p.tests.iter().filter(|b| b.required_features.contains(feature)) {
                bins.insert(bin.name.clone(), (None, why.clone()));
            }
            let unit = bins.entry(LIB.to_string()).or_insert_with(|| (Some(BTreeSet::new()), why.clone()));
            if let Some(modules) = &mut unit.0 {
                modules.insert(module.clone());
            }
            sel.features.insert(format!("{package}/{feature}"));
        }
        for (package, binary, module, path) in tests {
            if sel.packages.contains_key(&package) {
                continue;
            }
            let Some(p) = ws.package(&package) else { continue };
            for bin in p.tests.iter().filter(|b| binary.as_ref().is_none_or(|n| *n == b.name)) {
                let entry = sel
                    .partial
                    .entry(package.clone())
                    .or_default()
                    .entry(bin.name.clone())
                    .or_insert_with(|| (Some(BTreeSet::new()), format!("its test code changed ({path})")));
                match (&mut entry.0, &module) {
                    (Some(modules), Some(m)) => modules.extend(expand_modules(root, bin, m)),
                    // Once whole, it stays whole.
                    (None, _) => {}
                    (whole, None) => {
                        *whole = None;
                        entry.1 = format!("its test code changed ({path})");
                    }
                }
            }
        }
        // Features the selected test binaries need.
        let selected = sel.packages.keys().chain(sel.partial.keys()).cloned().collect::<Vec<_>>();
        for name in selected {
            let Some(p) = ws.package(&name) else { continue };
            for bin in &p.tests {
                let picked = sel.packages.contains_key(&name)
                    || sel.partial.get(&name).is_some_and(|b| b.contains_key(&bin.name));
                if picked {
                    sel.features
                        .extend(bin.required_features.iter().map(|f| format!("{name}/{f}")));
                }
            }
        }
        sel
    }

    /// Only the tests of the packages whose files changed, their test
    /// code's and features' parts, and this crate's (`--direct`: the
    /// pre-commit hook runs these and only builds the rest). A change that
    /// selects everything (the workspace's settings, the hooks, a build
    /// script) narrows to this crate's checks: the build covers it.
    pub fn direct(&self) -> Selection {
        let mut sel = self.clone();
        if sel.all.take().is_some() {
            sel.packages = BTreeMap::from([(CHECKS.to_string(), "always (the repository's rules)".to_string())]);
            sel.partial.clear();
            sel.features.clear();
            return sel;
        }
        sel.packages.retain(|name, _| name == CHECKS || self.changed.contains(name));
        let kept: BTreeSet<String> = sel.packages.keys().chain(sel.partial.keys()).cloned().collect();
        sel.features.retain(|f| kept.contains(f.split('/').next().unwrap_or_default()));
        sel
    }

    /// Arguments for `cargo nextest run` (after `run`): the packages to
    /// build and the filter of the tests to run; with `skip_heavy`, the
    /// fast tier (no `heavy::` tests).
    pub fn nextest_args(&self, skip_heavy: bool) -> Vec<String> {
        const NOT_HEAVY: &str = "!test(/^heavy::/)";
        let mut args = Vec::new();
        if self.all.is_some() {
            args.push("--workspace".to_string());
            if skip_heavy {
                args.extend(["-E".to_string(), NOT_HEAVY.to_string()]);
            }
            return args;
        }
        for name in self.packages.keys().chain(self.partial.keys()) {
            args.extend(["-p".to_string(), name.clone()]);
        }
        if !self.features.is_empty() {
            args.extend([
                "--features".to_string(),
                self.features.iter().cloned().collect::<Vec<_>>().join(","),
            ]);
        }
        let mut terms: Vec<String> = self.packages.keys().map(|p| format!("package({p})")).collect();
        for (package, bins) in &self.partial {
            for (bin, (modules, _)) in bins {
                let id = if bin == LIB {
                    format!("binary_id({package})")
                } else {
                    format!("binary_id({package}::{bin})")
                };
                let Some(modules) = modules else {
                    terms.push(id);
                    continue;
                };
                {
                    let tests: Vec<String> = modules
                        .iter()
                        .map(|m| format!("test(/^{m}::/)"))
                        .collect();
                    terms.push(format!("({id} & ({}))", tests.join(" | ")));
                }
            }
        }
        let mut filter = terms.join(" | ");
        if skip_heavy {
            filter = format!("({filter}) & {NOT_HEAVY}");
        }
        args.extend(["-E".to_string(), filter]);
        args
    }

    /// Arguments for `cargo test` (without nextest): whole packages, as
    /// libtest can't pick a package's binaries by filter; `--workspace`
    /// for everything.
    pub fn cargo_args(&self) -> Vec<String> {
        if self.all.is_some() {
            return vec!["--workspace".to_string()];
        }
        let mut args = Vec::new();
        for name in self.packages.keys().chain(self.partial.keys()) {
            args.extend(["-p".to_string(), name.clone()]);
        }
        if !self.features.is_empty() {
            args.extend([
                "--features".to_string(),
                self.features.iter().cloned().collect::<Vec<_>>().join(","),
            ]);
        }
        args
    }

    /// What was selected and why, for people.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let shown = 30;
        out.push_str(&format!("{} changed file(s):\n", self.files.len()));
        for (path, what) in self.files.iter().take(shown) {
            out.push_str(&format!("  {path} -> {what}\n"));
        }
        if self.files.len() > shown {
            out.push_str(&format!("  ... and {} more\n", self.files.len() - shown));
        }
        if let Some(why) = &self.all {
            out.push_str(&format!(
                "selected: EVERY TEST (the full suite; before a push its heavy tests too), because of: {why}\n"
            ));
            return out;
        }
        out.push_str("selected:\n");
        for (name, why) in &self.packages {
            out.push_str(&format!("  {name}: all tests; {why}\n"));
        }
        for (name, bins) in &self.partial {
            for (bin, (modules, why)) in bins {
                let Some(modules) = modules else {
                    out.push_str(&format!("  {name}: test binary `{bin}`; {why}\n"));
                    continue;
                };
                {
                    let list: Vec<String> = modules.iter().map(|m| format!("{m}::")).collect();
                    out.push_str(&format!("  {name}: `{bin}` {}; {why}\n", list.join(" ")));
                }
            }
        }
        out
    }
}

/// `module` of test binary `bin` and the modules of the same binary that
/// use it by path (`super::map_flows::Flow`), until none is added.
fn expand_modules(root: &Path, bin: &TestBinary, module: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::from([module.to_string()]);
    let Some(dir) = bin.module_dir() else { return out };
    let mut files = Vec::new();
    rust_files(&root.join(dir), &mut files);
    let modules: Vec<(String, String)> = files
        .iter()
        .filter_map(|f| {
            let rel = f.strip_prefix(root.join(dir)).ok()?;
            let stem = rel.to_string_lossy().replace('\\', "/");
            let stem = stem.strip_suffix(".rs")?;
            if stem == "main" {
                return None;
            }
            let path = stem.trim_end_matches("/mod").replace('/', "::");
            Some((path, fs::read_to_string(f).ok()?))
        })
        .collect();
    loop {
        let before = out.len();
        for (path, text) in &modules {
            if out.contains(path) {
                continue;
            }
            if out.iter().any(|m| names_module(text, m.rsplit("::").next().unwrap_or(m))) {
                out.insert(path.clone());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// Whether `text` uses module `stem` by path: `stem::` after a
/// non-identifier character.
fn names_module(text: &str, stem: &str) -> bool {
    let pat = format!("{stem}::");
    text.match_indices(&pat).any(|(i, _)| {
        text[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, dir: &str, deps: &[&str], dev: &[&str], tests: &[(&str, &[&str])]) -> Package {
        Package {
            name: name.into(),
            dir: dir.into(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            dev_deps: dev.iter().map(|s| s.to_string()).collect(),
            tests: tests
                .iter()
                .map(|(bin, features)| TestBinary {
                    name: bin.to_string(),
                    main: format!("{}{}tests/{bin}/main.rs", dir, if dir.is_empty() { "" } else { "/" }),
                    required_features: features.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
            feature_paths: Vec::new(),
        }
    }

    /// A small workspace shaped like ours.
    fn ws() -> Workspace {
        Workspace {
            packages: vec![
                pkg("source_formats", "source/formats", &[], &[], &[]),
                pkg("chimera_core", "chimera/core", &["source_formats"], &[], &[]),
                pkg("chimera_present", "chimera/present", &["chimera_core"], &[], &[("it", &[])]),
                Package {
                    feature_paths: vec![("src/ui/".into(), "ui".into())],
                    ..pkg("cs_source", "games/cs_source", &["chimera_core", "chimera_present"], &["crc"], &[("it", &[]), ("ui", &["ui"])])
                },
                pkg("cs_source_app", "apps/cs_source", &["cs_source"], &[], &[("it", &[])]),
                pkg("cs_source_tools", "games/cs_source/tools", &["cs_source_app"], &[], &[]),
                pkg("mashup", "", &["cs_source_app"], &[], &[("it", &["present"])]),
                pkg(CHECKS, "checks", &[], &[], &[("it", &[])]),
            ],
        }
    }

    fn select(files: &[&str]) -> Selection {
        let files: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        // A root where no file exists: test modules count as deleted
        // unless a test says otherwise.
        Selection::new(&ws(), Path::new("/nonexistent"), &files)
    }

    #[test]
    fn direct_keeps_only_changed_packages() {
        let s = select(&["chimera/core/src/lib.rs", "games/cs_source/src/ui/hud.rs"]);
        assert!(s.packages.contains_key("cs_source_app"), "{s:?}");
        let d = s.direct();
        // cs_source (its `ui` part changed) stays, whole: chimera_core's
        // change selected all of it.
        assert_eq!(d.packages.keys().collect::<Vec<_>>(), ["chimera_core", "cs_source", CHECKS]);
        assert!(d.features.iter().all(|f| f.starts_with("cs_source/")), "{:?}", d.features);
        // Alone, the `ui` part: only its binaries with the feature.
        let d = select(&["games/cs_source/src/ui/hud.rs"]).direct();
        assert_eq!(d.packages.keys().collect::<Vec<_>>(), [CHECKS]);
        assert!(d.partial.contains_key("cs_source"), "{d:?}");
        assert_eq!(d.features.iter().collect::<Vec<_>>(), ["cs_source/ui"]);
        // What selects everything narrows to the repository checks.
        let d = select(&[".githooks/pre-commit"]).direct();
        assert!(d.all.is_none());
        assert_eq!(d.cargo_args(), ["-p", CHECKS]);
    }

    #[test]
    fn classifies_paths() {
        let ws = ws();
        assert!(matches!(classify(&ws, "Cargo.lock", true), Change::All(_)));
        assert!(matches!(classify(&ws, "apps/cs_source/build.rs", true), Change::All(_)));
        assert!(matches!(classify(&ws, ".config/nextest.toml", true), Change::All(_)));
        assert!(matches!(classify(&ws, "checks/src/affected.rs", true), Change::All(_)));
        assert!(matches!(classify(&ws, "some/unknown/file", true), Change::All(_)));
        assert_eq!(classify(&ws, "docs/ARCHITECTURE.md", true), Change::Docs);
        assert_eq!(classify(&ws, "chimera/core/README.md", true), Change::Docs);
        assert_eq!(classify(&ws, "specs/cs_source/ropes.md", true), Change::Docs);
        assert_eq!(classify(&ws, "tools/doctor", true), Change::Docs);
        assert_eq!(classify(&ws, ".claude/settings.json", true), Change::Docs);
        assert_eq!(
            classify(&ws, "games/cs_source/src/rules.rs", true),
            Change::Package("cs_source".into())
        );
        // A folder only a feature compiles (`feature-paths`).
        assert_eq!(
            classify(&ws, "games/cs_source/src/ui/radar.wesl", true),
            Change::Feature {
                package: "cs_source".into(),
                feature: "ui".into(),
                module: "ui".into()
            }
        );
        // The deepest member: cs_source's tools aren't cs_source.
        assert_eq!(
            classify(&ws, "games/cs_source/tools/src/bin/refcmp.rs", true),
            Change::Package("cs_source_tools".into())
        );
        assert_eq!(classify(&ws, "src/lib.rs", true), Change::Package("mashup".into()));
        assert_eq!(classify(&ws, "tools/refcmp/de_dust2.toml", true), Change::Package("cs_source_tools".into()));
        assert_eq!(
            classify(&ws, "apps/cs_source/tests/it/heavy/map_surf.rs", true),
            Change::Tests {
                package: "cs_source_app".into(),
                binary: Some("it".into()),
                module: Some("heavy::map_surf".into())
            }
        );
        assert_eq!(
            classify(&ws, "apps/cs_source/tests/it/main.rs", true),
            Change::Tests {
                package: "cs_source_app".into(),
                binary: Some("it".into()),
                module: None
            }
        );
        // Deleted: the whole binary.
        assert_eq!(
            classify(&ws, "apps/cs_source/tests/it/weapons.rs", false),
            Change::Tests {
                package: "cs_source_app".into(),
                binary: Some("it".into()),
                module: None
            }
        );
        // A helper outside any binary's folder: all of the package's tests.
        assert_eq!(
            classify(&ws, "games/cs_source/tests/common/mod.rs", true),
            Change::Tests {
                package: "cs_source".into(),
                binary: None,
                module: None
            }
        );
    }

    #[test]
    fn a_base_crate_selects_everything_above_it() {
        let s = select(&["chimera/core/src/core.rs"]);
        assert!(s.all.is_none());
        let names: Vec<&str> = s.packages.keys().map(|s| s.as_str()).collect();
        assert_eq!(
            names,
            ["chimera_core", "chimera_present", "cs_source", "cs_source_app", "cs_source_tools", "mashup", CHECKS]
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert!(s.features.contains("cs_source/ui"));
        assert!(s.features.contains("mashup/present"));
    }

    #[test]
    fn a_feature_part_selects_its_binaries_and_whats_above() {
        let s = select(&["games/cs_source/src/ui/game_hud.rs"]);
        assert!(s.all.is_none());
        let names: Vec<&str> = s.packages.keys().map(|s| s.as_str()).collect();
        assert_eq!(names, ["cs_source_app", "cs_source_tools", "mashup", CHECKS]);
        let bins = &s.partial["cs_source"];
        assert_eq!(bins.keys().collect::<Vec<_>>(), [LIB, "ui"]);
        assert!(bins["ui"].0.is_none(), "the whole `ui` binary");
        assert_eq!(bins[LIB].0.as_ref().unwrap().iter().collect::<Vec<_>>(), ["ui"]);
        assert!(s.features.contains("cs_source/ui"));
        assert_eq!(
            s.nextest_args(false).last().unwrap(),
            "package(cs_source_app) | package(cs_source_tools) | package(mashup) | package(repo_checks) \
             | (binary_id(cs_source) & (test(/^ui::/))) | binary_id(cs_source::ui)"
        );
        // With the rest of the crate changed too: all of it.
        let s = select(&["games/cs_source/src/ui/game_hud.rs", "games/cs_source/src/rules.rs"]);
        assert!(s.packages.contains_key("cs_source"));
        assert!(s.partial.is_empty());
    }

    #[test]
    fn a_leaf_crate_selects_itself_and_the_checks() {
        let s = select(&["games/cs_source/tools/src/lib.rs"]);
        let names: Vec<&str> = s.packages.keys().map(|s| s.as_str()).collect();
        assert_eq!(names, ["cs_source_tools", CHECKS]);
    }

    #[test]
    fn docs_select_only_the_checks() {
        let s = select(&["docs/OBSERVABILITY.md", "CLAUDE.md"]);
        assert!(s.all.is_none());
        let names: Vec<&str> = s.packages.keys().map(|s| s.as_str()).collect();
        assert_eq!(names, [CHECKS]);
        assert!(s.partial.is_empty());
        assert_eq!(s.nextest_args(false), ["-p", CHECKS, "-E", "package(repo_checks)"]);
        // The probes' plugins and drivers: no crate reads them.
        let s = select(&["tools/hl2dm_probe/cases.py", "tools/hl2dm_probe/chimera_probe.sp"]);
        assert!(s.all.is_none());
        assert_eq!(s.packages.keys().collect::<Vec<_>>(), [CHECKS]);
    }

    #[test]
    fn test_code_selects_its_binary_only() {
        let s = select(&["games/cs_source/tests/ui/main.rs"]);
        assert_eq!(s.packages.keys().collect::<Vec<_>>(), [CHECKS]);
        assert!(s.partial["cs_source"].contains_key("ui"));
        assert!(!s.partial["cs_source"].contains_key("it"));
        // The binary's feature is turned on, or cargo wouldn't build it.
        assert_eq!(s.features.iter().collect::<Vec<_>>(), ["cs_source/ui"]);
        let args = s.nextest_args(true);
        assert_eq!(
            args.last().unwrap(),
            "(package(repo_checks) | binary_id(cs_source::ui)) & !test(/^heavy::/)"
        );
    }

    #[test]
    fn test_code_under_a_changed_package_runs_with_it() {
        let s = select(&["games/cs_source/tests/ui/main.rs", "games/cs_source/src/lib.rs"]);
        assert!(s.partial.is_empty());
        assert!(s.packages.contains_key("cs_source"));
    }

    /// A lockfile: (name, version, source, checksum, dependencies).
    fn lock(packages: &[(&str, &str, &str, &str, &[&str])]) -> String {
        let mut out = String::from("version = 4\n");
        for (name, version, source, sum, deps) in packages {
            out.push_str(&format!("\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"));
            if !source.is_empty() {
                out.push_str(&format!("source = \"{source}\"\nchecksum = \"{sum}\"\n"));
            }
            let deps: Vec<String> = deps.iter().map(|d| format!("\"{d}\"")).collect();
            out.push_str(&format!("dependencies = [{}]\n", deps.join(", ")));
        }
        out
    }

    const REG: &str = "registry+https://github.com/rust-lang/crates.io-index";

    #[test]
    fn a_lockfile_change_selects_the_members_that_build_on_it() {
        let before = lock(&[
            ("source_formats", "0.1.0", "", "", &["memchr", "workspace-hack"]),
            ("chimera_core", "0.1.0", "", "", &["source_formats", "serde", "workspace-hack"]),
            ("cs_source_tools", "0.1.0", "", "", &["image", "workspace-hack"]),
            ("workspace-hack", "0.1.0", "", "", &["memchr", "serde", "image"]),
            ("memchr", "2.7.0", REG, "a", &[]),
            ("serde", "1.0.0", REG, "b", &[]),
            ("image", "0.25.0", REG, "c", &["png 0.17.0"]),
            ("png", "0.17.0", REG, "d", &[]),
            ("png", "0.18.0", REG, "e", &[]),
        ]);
        // png 0.17 -> 0.17.1 under image (two versions: named with one).
        let after = before
            .replace("\"png 0.17.0\"", "\"png 0.17.1\"")
            .replace("version = \"0.17.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"d\"",
                     "version = \"0.17.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"f\"");
        let hit = lock_affected(&before, &after).unwrap();
        // Not through the workspace-hack: only the tools use image.
        assert_eq!(hit.keys().collect::<Vec<_>>(), ["cs_source_tools"]);
        assert_eq!(
            hit["cs_source_tools"].iter().collect::<Vec<_>>(),
            ["image 0.25.0", "png 0.17.0", "png 0.17.1"]
        );
        // memchr's checksum: what builds on source_formats.
        let after = before.replace("checksum = \"a\"", "checksum = \"z\"");
        let hit = lock_affected(&before, &after).unwrap();
        assert_eq!(hit.keys().collect::<Vec<_>>(), ["chimera_core", "source_formats"]);
        // Unchanged: nothing.
        assert!(lock_affected(&before, &before).unwrap().is_empty());

        // In a selection: those members and what depends on them, not
        // everything.
        let members = Ok(BTreeMap::from([(
            "source_formats".to_string(),
            BTreeSet::from(["memchr 2.7.1".to_string()]),
        )]));
        let files = vec!["Cargo.lock".to_string()];
        let s = Selection::with_lock(&ws(), Path::new("/nonexistent"), &files, Some(&members));
        assert!(s.all.is_none());
        assert!(s.packages["source_formats"].contains("memchr 2.7.1"), "{:?}", s.packages);
        assert!(s.packages.contains_key("cs_source"));
        // No member affected: the checks only.
        let none = Ok(BTreeMap::new());
        let s = Selection::with_lock(&ws(), Path::new("/nonexistent"), &files, Some(&none));
        assert_eq!(s.packages.keys().collect::<Vec<_>>(), [CHECKS]);
        // Couldn't compare: everything, saying why.
        let err = Err("no HEAD".to_string());
        let s = Selection::with_lock(&ws(), Path::new("/nonexistent"), &files, Some(&err));
        assert!(s.all.as_ref().unwrap().contains("no HEAD"));
        assert!(s.summary().contains("EVERY TEST"));
    }

    #[test]
    fn everything_says_every_reason() {
        let s = select(&[".config/nextest.toml", ".githooks/pre-push"]);
        let why = s.all.unwrap();
        assert!(why.contains(".config/nextest.toml") && why.contains(".githooks/pre-push"), "{why}");
    }

    #[test]
    fn everything_wins() {
        let s = select(&["docs/a.md", "Cargo.lock"]);
        assert!(s.all.is_some());
        assert_eq!(s.nextest_args(true), ["--workspace", "-E", "!test(/^heavy::/)"]);
        assert_eq!(s.cargo_args(), ["--workspace"]);
    }

    #[test]
    fn modules_that_use_a_changed_module_run_too() {
        let dir = std::env::temp_dir().join(format!("repo-checks-affected-{}", std::process::id()));
        let it = dir.join("apps/cs_source/tests/it");
        fs::create_dir_all(it.join("heavy")).unwrap();
        fs::write(it.join("main.rs"), "mod heavy { mod map_flows; mod map_courses; mod map_surf; }").unwrap();
        fs::write(it.join("heavy/map_flows.rs"), "pub struct Flow;").unwrap();
        fs::write(it.join("heavy/map_courses.rs"), "use super::map_flows::{Flow};").unwrap();
        fs::write(it.join("heavy/map_surf.rs"), "// not_map_flows::x").unwrap();
        let files = vec!["apps/cs_source/tests/it/heavy/map_flows.rs".to_string()];
        let s = Selection::new(&ws(), &dir, &files);
        let modules = s.partial["cs_source_app"]["it"].0.as_ref().unwrap();
        assert_eq!(
            modules.iter().collect::<Vec<_>>(),
            ["heavy::map_courses", "heavy::map_flows"]
        );
        let args = s.nextest_args(false);
        assert_eq!(
            args.last().unwrap(),
            "package(repo_checks) | (binary_id(cs_source_app::it) & (test(/^heavy::map_courses::/) | test(/^heavy::map_flows::/)))"
        );
        // With its main.rs changed as well, the whole binary.
        let files = vec![
            "apps/cs_source/tests/it/heavy/map_flows.rs".to_string(),
            "apps/cs_source/tests/it/main.rs".to_string(),
        ];
        let s = Selection::new(&ws(), &dir, &files);
        assert!(s.partial["cs_source_app"]["it"].0.is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
