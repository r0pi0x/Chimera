//! TF2's guns' kill icons from the asset store (`game::hud::
//! kill_icon_pictures`, what `ui::systems::register_kill_icons` registers
//! for the kill feed): with TF2 read from the store, its manifest naming
//! `scripts/mod_textures.txt` and the sheet but their records not yet
//! fetched, nothing is read (no download; the feed writes names and the
//! registration tries again later); once the records are in the store
//! (the `players` set arrived with a match), each gun's cell of the sheet
//! is its picture. A made-up sheet and cells (hud.md 3.6's shape), never
//! the game's.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use chimera_core::mount::{
    Mount,
    assets::{CachedFiles, Fetcher, FileRecord, Manifest, Store, sha256_hex},
};
use tf2::game::{hud, weapons};

const MOD_TEXTURES: &str = r#""sprites/640_hud"
{
    TextureData
    {
        "scattergun" { "dfile" "hud/d_images" "dnegfile" "hud/dneg_images" "x" "0" "y" "0" "width" "4" "height" "2" }
        "minigun" { "file" "hud/d_images" "x" "4" "y" "2" "width" "4" "height" "2" }
    }
}"#;

/// Counts what it is asked; hands out nothing.
struct Counting(AtomicUsize);

impl Fetcher for Counting {
    fn fetch(&self, _: &Store, _: &str, _: u64) -> std::io::Result<Vec<u8>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(std::io::ErrorKind::NotFound.into())
    }
}

/// An 8 x 8 sheet whose pixel (x, y) is (x, y, 7, 255).
fn sheet() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
    vtf::vtf::VTF::create(image::DynamicImage::ImageRgba8(img), vtf::ImageFormat::Rgba8888).expect("a sheet")
}

#[test]
fn kill_icons_read_from_the_store_once_their_files_are_here() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("tf2-kill-icons-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let files = [("scripts/mod_textures.txt", MOD_TEXTURES.as_bytes().to_vec()), ("materials/hud/d_images.vtf", sheet())];
    let mut manifest = Manifest::new("tf2", "test", None);
    for (path, bytes) in &files {
        let record = FileRecord { id: sha256_hex(bytes), size: bytes.len() as u64, stored: 1 };
        manifest.files.insert(path.to_string(), record);
        manifest.sets.entry("players".into()).or_default().insert(path.to_string());
    }
    let fetcher = Arc::new(Counting(AtomicUsize::new(0)));
    let mount_now = || {
        let mut mount = Mount::default();
        mount.push(CachedFiles::new(store.clone(), &manifest).with_fetcher(Some(fetcher.clone() as Arc<dyn Fetcher>)));
        mount
    };

    // Named, not here: nothing read, nothing fetched.
    assert!(hud::kill_icon_pictures(&mount_now()).is_none(), "not here yet");
    // The script here, the sheet not yet: still waiting.
    store.put(&files[0].1).unwrap();
    assert!(hud::kill_icon_pictures(&mount_now()).is_none(), "the sheet isn't here yet");
    assert_eq!(fetcher.0.load(Ordering::SeqCst), 0, "never a download");

    // The set arrived.
    store.put(&files[1].1).unwrap();
    let icons = hud::kill_icon_pictures(&mount_now()).expect("here now");
    assert_eq!(fetcher.0.load(Ordering::SeqCst), 0);
    assert_eq!(icons.keys().collect::<Vec<_>>(), [weapons::MINIGUN, weapons::SCATTERGUN]);
    let scatter = &icons[weapons::SCATTERGUN];
    assert_eq!((scatter.width, scatter.height), (4, 2));
    assert_eq!(&scatter.rgba8[..8], &[0, 0, 7, 255, 1, 0, 7, 255], "the cell at (0, 0)");
    let minigun = &icons[weapons::MINIGUN];
    assert_eq!(&minigun.rgba8[..4], &[4, 2, 7, 255], "the cell at (4, 2)");

    // A mount without TF2's script at all: none yet (its files may come).
    assert!(hud::kill_icon_pictures(&Mount::default()).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
