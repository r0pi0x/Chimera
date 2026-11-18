# CLAUDE.md

Chimera is a Rust/Bevy 0.20 + Avian3D 0.8 monorepo rebuilding first-person games from the player's own installs and mixing their parts (**Lucker Party**). Layers: `chimera/` → loaders (`source/`, `lithtech/`, `blam/`) → `games/` → `apps/`.

## Key Docs
- Contract, rules, setup, asset store: [AGENTS.md](AGENTS.md)
- Overview & install: [README.md](README.md)
- Architecture & crate map: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Design decisions: [docs/DESIGN.md](docs/DESIGN.md)
- Testing, screenshots, remote ECS: [docs/OBSERVABILITY.md](docs/OBSERVABILITY.md)
- Porting a game: [docs/new-game.md](docs/new-game.md)
- Specs & third-party credits: [specs/README.md](specs/README.md), [THIRD_PARTY.md](THIRD_PARTY.md)

## Hard Rules (Enforced by `cargo test -p repo_checks`)
1. **No game assets or keys**: Never commit game assets, extracted files, or `chimera.local.toml` secrets.
2. **Specs or listed sources only**: Copy/adapt only from [THIRD_PARTY.md](THIRD_PARTY.md) with credit header; original-game behavior enters only via [specs/](specs/README.md) (no code blocks).
3. **Linux & Windows**: LF line endings; check pinned Bevy `0.20.0` / Avian3D `0.8` crate sources in `~/.cargo/registry/src/` instead of guessing.
4. **Commit identity**: `r0pi0x <339403489+r0pi0x@users.noreply.github.com>` with `TZ=UTC`; never skip hooks.

## Common Commands
```
cargo chk                                      # check --workspace --all-targets
cargo t-fast                                   # fast tier (--skip heavy::)
cargo checks                                   # repo_checks (--locked)
cargo test -p <crate> --features dev           # one crate's tests
cargo run --features dev                       # run cs_source (default-run)
cargo run --features dev -- --help             # screenshot, offscreen, spawn, look
cargo run -q -p repo_checks --bin affected -- --base main
cargo run -q -p repo_checks --bin docs_index   # refresh docs/modules.md & tech-debt.md
cargo hakari generate                          # after dependency changes
```
