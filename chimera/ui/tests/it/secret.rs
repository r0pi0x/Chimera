//! Values hidden by default (`chimera_ui::secret`): the masks, which words
//! are addresses, and a reveal forgotten once no node shows it.

use bevy::prelude::*;
use chimera_ui::secret::{self, Reveals, Secret};

#[test]
fn masks_hide_addresses_and_keep_a_keys_groups() {
    assert_eq!(secret::mask("203.0.113.7:27015"), secret::mask("x"), "a fixed run: nothing of the shape");
    assert_eq!(secret::mask_chars("AB12-CD34"), "••••-••••");
    for a in ["203.0.113.7", "203.0.113.7:27015", "[2001:db8::1]:27015", "hub.example.net:27080", "localhost:27015"] {
        assert!(secret::looks_like_address(a), "{a}");
    }
    for a in ["Friday Night Hub", "12:30", "UDP", "27015"] {
        assert!(!secret::looks_like_address(a), "{a}");
    }
    let line = secret::mask_addresses("UDP 27015 forwarded by router 192.168.1.1; friends connect to 203.0.113.7:27015");
    assert!(!line.contains("192.168") && !line.contains("203.0"), "{line}");
    assert!(line.starts_with("UDP 27015 forwarded by router ") && line.contains("; friends connect to "), "{line}");
    assert_eq!(secret::name_or_masked("Back room", false), "Back room");
    assert_eq!(secret::name_or_masked("203.0.113.7:27015", true), "203.0.113.7:27015");
}

#[test]
fn a_reveal_is_forgotten_when_no_node_shows_it() {
    let mut app = App::new();
    app.init_resource::<Reveals>().add_systems(Update, secret::forget_reveals);
    let shown = app.world_mut().spawn(Secret("a".into())).id();
    app.world_mut().resource_mut::<Reveals>().toggle("a");
    app.update();
    assert!(app.world().resource::<Reveals>().shown("a"), "kept while drawn");
    app.world_mut().despawn(shown);
    app.update();
    assert!(!app.world().resource::<Reveals>().shown("a"), "the next screen starts hidden");
}
