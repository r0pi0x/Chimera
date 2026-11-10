//! The CS:S dedicated server's integration tests: the server as the
//! binary composes it (`ServerPlugin`, its command line through `parse`),
//! clients over UDP on loopback (ports the system picks, never 27015).

mod dedicated;
