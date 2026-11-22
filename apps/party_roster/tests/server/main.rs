//! Lucker Party's dedicated server's integration tests: the server as the
//! binary composes it (`ServerPlugin`, its command line through `parse`),
//! headless, scripted soldiers in place of clients; `heavy::` on the
//! user's installs (de_dust2 dressed for Combat Arms).

mod support;
mod greybox;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod dust2;
    mod pools;
    mod spawns;
    mod warehouse;
}
