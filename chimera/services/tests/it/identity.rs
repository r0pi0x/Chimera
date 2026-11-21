//! Hub identities (`chimera_services::identity`): a fresh hub is "Chimera
//! Hub"; a named identity is its own community (its own accounts,
//! invites and ticket key) and switching back finds the first one as it
//! was.

use chimera_assets::account::CheckRequest;
use chimera_services::{
    accounts::{Accounts, Clock, InviteSpec, Options, Paths},
    identity::{self, DEFAULT_NAME, MAIN},
};

use super::scratch;

fn open(store: &std::path::Path) -> Accounts {
    Accounts::open(Paths::active(store).unwrap(), Options::default(), Clock::default()).unwrap()
}

fn check(accounts: &Accounts, code: &str) -> Result<String, u16> {
    accounts.check(&CheckRequest { code: code.into(), username: None }, None).map(|a| a.hub.name).map_err(|r| r.status)
}

#[test]
fn identities_are_separate_communities_and_switch_back_intact() {
    let store = scratch("identities");
    let spec = InviteSpec { intent: Some("the owner".into()), role: Some("owner".into()), ..Default::default() };

    // A hub from before identities: `main`, the store's own folders, the default name.
    let main = open(&store);
    assert_eq!(main.paths.db(), store.join("server").join("accounts").join("accounts.db"));
    let owner_code = main.invite(&spec).unwrap().code;
    assert_eq!(check(&main, &owner_code).unwrap(), DEFAULT_NAME);
    let main_keys = main.ticket_keys();
    identity::set_name(&store, MAIN, "Owner's hub").unwrap();
    assert_eq!(check(&main, &owner_code).unwrap(), "Owner's hub", "the name is read live");
    drop(main);

    // A second identity: its own folder, name and accounts; `use` switches.
    identity::create(&store, "friends", "").unwrap();
    assert!(identity::create(&store, "friends", "x").is_err(), "ids are unique");
    assert!(identity::activate(&store, "nobody").is_err());
    identity::activate(&store, "friends").unwrap();
    let list = identity::list(&store).unwrap();
    assert_eq!(list.iter().map(|i| (i.id.as_str(), i.name.as_str(), i.active)).collect::<Vec<_>>(), [
        (MAIN, "Owner's hub", false),
        ("friends", DEFAULT_NAME, true)
    ]);
    let friends = open(&store);
    assert_eq!(friends.paths.db(), store.join("server").join("identities").join("friends").join("accounts").join("accounts.db"));
    assert_eq!(check(&friends, &owner_code), Err(404), "the other identity's invite is unknown here");
    let theirs = friends.invite(&InviteSpec { intent: Some("a tester".into()), ..Default::default() }).unwrap().code;
    assert_eq!(check(&friends, &theirs).unwrap(), DEFAULT_NAME);
    assert_ne!(friends.ticket_keys().to_text(), main_keys.to_text(), "its own ticket key: no tickets cross");
    let published = std::fs::read_to_string(friends.paths.ticket_pub()).unwrap();
    assert_eq!(published, friends.ticket_keys().to_text(), "the active identity's key is the one the services check");
    drop(friends);

    // Back to `main`: the owner's unused invite still works; services.toml keeps its other keys.
    std::fs::write(store.join("server").join("services.toml"), "[front]\nlisten = \"0.0.0.0:9\"\n[hub]\nidentity = \"friends\"\n").unwrap();
    identity::activate(&store, MAIN).unwrap();
    let config = chimera_services::config::ServicesConfig::load(&store).unwrap();
    assert_eq!((config.front.listen.as_deref(), config.hub.identity), (Some("0.0.0.0:9"), None));
    let main = open(&store);
    assert_eq!(check(&main, &owner_code).unwrap(), "Owner's hub");
    assert_eq!(check(&main, &theirs), Err(404));
    assert_eq!(main.ticket_keys().to_text(), main_keys.to_text());
}
