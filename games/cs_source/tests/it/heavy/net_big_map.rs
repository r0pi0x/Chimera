//! A community-map-sized download over the connection (backlog 0,
//! playtest 2026-10-10: "maps/mg_swag_multigames_v1.bsp is 141.5 MB, more
//! than the server sends (net_maxfilesize 64)"). A 150 MB map file goes
//! from a server at its default settings to a client without it, in
//! `NetSim` (`games/cs_source/tests/it/net_maps.rs`'s maps built in code): sent by
//! default (`net_maxfilesize` 512), the loading dialog's bytes moving
//! every second of it, the server's game going on, and the copy cached,
//! checked and played. Heavy: 150 MB through the in-memory link takes a
//! while in a debug build.

use std::path::PathBuf;

use crate::stack::net::client::{JoinStage, Joined};

use crate::net_maps::{add_client, file_bytes, loaded, progress, server, stages, write_map};

const SIZE: usize = 150 * 1024 * 1024;

#[test]
fn a_150_mb_map_downloads_over_the_connection_by_default() {
    // Big files go under the build's own scratch folder, not /tmp.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("net-big-map-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let install = dir.join("server");
    let bytes = file_bytes("big", 1, SIZE);
    write_map(&install, "big", &bytes);
    let mut sim = server(install.clone(), "cs_source:big", 77);
    let empty = dir.join("client");
    std::fs::create_dir_all(&empty).unwrap();
    let cache = dir.join("cache");
    let i = add_client(&mut sim, &empty, Some(&cache));
    // The checking and saving of the 150 MB (a task) lands on the tick it
    // is asked for, not when the machine's load allows: the frames counted
    // below are the transfer's, whatever the load.
    sim.server.repeatable();
    sim.clients[i].repeatable();

    // At the tick rate: the bytes shown grow every second (no stall,
    // nothing near the 30 s timeout without traffic), the server's ticks
    // go on.
    let hz = sim.server.tick_hz() as u64;
    let (mut last, mut since, mut most_still, mut frames) = (0u64, 0u64, 0u64, 0u64);
    let ticks_before = sim.server.tick();
    let ok = sim.until(hz * 600, |s| {
        let w = s.clients[i].app.world();
        if let Some(d) = &w.resource::<crate::stack::net::client::JoinProgress>().download {
            assert!(d.total == SIZE as u64, "the dialog's total: {}", d.total);
            if d.done > last {
                last = d.done;
                since = 0;
            } else if d.done < d.total {
                since += 1;
                most_still = most_still.max(since);
            }
            // The transfer's frames: not the check and the save after it.
            if d.done < d.total {
                frames += 1;
            }
        }
        w.contains_resource::<Joined>()
    });
    let p = progress(&sim, i);
    println!(
        "{:?}, failure {:?}; {frames} frames downloading ({:.1} s at {hz} Hz), longest without new bytes {most_still}",
        stages(&p),
        p.failure,
        frames as f64 / hz as f64
    );
    assert!(ok, "joined after downloading 150 MB");
    assert!(stages(&p).contains(&JoinStage::Downloading));
    assert_eq!(last, SIZE as u64, "every byte shown arriving");
    assert!(
        most_still < hz,
        "the download never stood still a second ({most_still} frames)"
    );
    assert!(
        frames < hz * 120,
        "150 MB in under two minutes of game time ({:.1} s)",
        frames as f64 / hz as f64
    );
    assert!(
        sim.server.tick() - ticks_before >= frames,
        "the server's game went on meanwhile"
    );
    let file = cache.join("maps/big.bsp");
    assert_eq!(std::fs::metadata(&file).unwrap().len(), SIZE as u64);
    assert!(std::fs::read(&file).unwrap() == bytes, "the server's file");
    assert_eq!(loaded(&sim, i).unwrap().name, "big");
    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}
