use super::*;
    use super::*;

    #[test]
    fn the_surface_table_parses() {
        let t = CaSurfaces::built_in();
        assert_eq!(t.get("Wood").map(|s| s.damage), Some(0.5));
        assert_eq!(t.get("plywood").map(|s| s.damage), Some(0.6));
        assert_eq!(t.get("water").map(|s| s.thickness), Some(0.0));
        assert!(t.get("concrete").is_none());
        assert_eq!(t.get("unknown").map(|s| s.damage), Some(0.6));
    }

    #[test]
    fn spheres_are_named_once() {
        for set in [*MALE, *FEMALE] {
            for (i, s) in set.iter().enumerate() {
                assert!(set[..i].iter().all(|o| o.name != s.name), "{}", s.name);
            }
        }
    }

