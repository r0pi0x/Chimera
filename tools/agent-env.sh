# shellcheck shell=bash
# Source this in every agent shell before building or testing:
#
#   source tools/agent-env.sh
#   agent_run cargo test -p chimera_world --features dev
#
# What it does (measured: docs/OBSERVABILITY.md "Several agents at once"):
#  * Joins one shared GNU-make-style jobserver FIFO (default: one token per
#    CPU thread) via MAKEFLAGS. cargo and rustc of every agent on the box then
#    draw compile jobs from the same pool, so 4 simultaneous builds use ~16
#    rustc processes instead of ~48. This is the control that matters.
#  * agent_run runs a command inside the user slice agents.slice
#    (CPUWeight=50, MemoryHigh=24G), so agents yield to the desktop and
#    anything interactive instead of starving it.
#  * Points cargo at one box-wide sccache (RUSTC_WRAPPER, port 4227, config in
#    ~/.config/sccache/config) with CARGO_INCREMENTAL=0, so a fresh worktree's
#    first build is mostly cache hits (docs/OBSERVABILITY.md). In a build slot
#    (tools/agent-slot.sh: .claude/slots/*, a fixed path whose target/ stays
#    warm) cargo's incremental builds stay on instead: the registry crates
#    still come from sccache, the workspace crates rebuild incrementally. The
#    server is
#    started WITHOUT MAKEFLAGS, and tools/sccache-heal.sh restarts it if it
#    has leaked its own jobserver tokens (the "hung at 0% CPU" failure).
# The FIFO only holds its tokens while some process has it open, so a small
# holder process (`sleep infinity`) is started once, race-free under flock.
# A cargo killed mid-build (a tool timeout) takes its tokens with it; each
# `source` runs tools/jobserver-heal.sh, which puts lost ones back (without
# it the FIFO drained to 0 in a day and every agent's build ran one rustc
# at a time: docs/performance.md, "Iteration speed").
# Override: AGENT_JOBS (token count), AGENT_JS_DIR (where the FIFO lives),
# AGENT_JS_HEAL_EVERY (seconds between jobserver checks, default 20),
# AGENT_NO_SLICE=1 (skip the slice), AGENT_NO_SCCACHE=1 (build without it),
# SCCACHE_BIN (the sccache to use), AGENT_INCREMENTAL=1|0 (incremental builds
# on or off; default on in a slot only), AGENT_NO_MOLD=1 (in a slot, link
# with rustc's default linker instead of mold).

_agent_env_init() {
  # Non-login shells (agent tool sandboxes) often lack these; the user bus needs them.
  [ -z "$XDG_RUNTIME_DIR" ] && [ -d "/run/user/$(id -u)" ] && export XDG_RUNTIME_DIR="/run/user/$(id -u)"
  local dir="${AGENT_JS_DIR:-${XDG_RUNTIME_DIR:-/tmp}/agent-jobserver}"
  local jobs="${AGENT_JOBS:-$(nproc)}"
  local fifo="$dir/jobserver.fifo" pidf="$dir/holder.pid" pid
  mkdir -p "$dir" || return 1
  (
    flock 5
    pid=$(cat "$pidf" 2>/dev/null)
    if [ -z "$pid" ] || [ ! -d "/proc/$pid" ] || [ ! -p "$fifo" ]; then
      rm -f "$fifo"
      mkfifo "$fifo" || exit 1
      # The holder keeps the FIFO open read-write so the tokens survive
      # between cargo runs. Its fds exclude the lock fd (5).
      setsid bash -c '
        exec 9<>"$1"
        printf "%*s" "$2" "" | tr " " "+" >&9
        echo $$ >"$3"
        exec sleep infinity' _ "$fifo" "$jobs" "$pidf" 5>&- >/dev/null 2>&1 &
      for _ in $(seq 1 50); do [ "$(cat "$pidf" 2>/dev/null)" ] && [ -d "/proc/$(cat "$pidf")" ] && break; sleep 0.1; done
    fi
    # Tokens a killed cargo held are gone for good: put them back
    # (tools/jobserver-heal.sh; its fds exclude the lock fd too).
    "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/jobserver-heal.sh" "$fifo" "$jobs" "$(cat "$pidf" 2>/dev/null)" 5>&- || true
  ) 5>"$dir/lock" || return 1
  export MAKEFLAGS="--jobserver-auth=fifo:$fifo -j$jobs"
  export AGENT_ENV=1
}

# Cache compiles in one sccache server shared by every worktree. sccache keys
# on the full rustc command line and on every CARGO_* variable of the client
# (CARGO_TARGET_DIR included), so builds in different worktrees share entries
# only if those match: the target dir must be the default <worktree>/target
# (hence the unset) and builds must not be incremental (sccache cannot cache
# those). Registry crates, the big part of a first build, then hit across
# worktrees; workspace crates hit only when the path is the same.
# In a slot the path is the same each time and target/ is kept, so
# incremental compilation (cargo's default, which only applies to workspace
# crates) beats caching them; registry crates are never incremental, so their
# entries still hit. AGENT_INCREMENTAL=1 or 0 overrides the choice.
_agent_sccache_init() {
  [ -n "$AGENT_NO_SCCACHE" ] && return 0
  local bin="${SCCACHE_BIN:-$HOME/.local/sccache-0.19/bin/sccache}"
  [ -x "$bin" ] || { echo "agent-env: $bin missing (cargo install sccache --locked --no-default-features --root ~/.local/sccache-0.19); building uncached" >&2; return 0; }
  [ -n "$CARGO_TARGET_DIR" ] && echo "agent-env: unsetting CARGO_TARGET_DIR ($CARGO_TARGET_DIR): sccache would hash it; cargo uses <worktree>/target" >&2
  unset CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR
  local here; here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  export RUSTC_WRAPPER="$bin" SCCACHE_SERVER_PORT="${SCCACHE_SERVER_PORT:-4227}"
  local incremental="${AGENT_INCREMENTAL:-}"
  if [ -z "$incremental" ]; then
    case "$here" in */.claude/slots/*) incremental=1 ;; *) incremental=0 ;; esac
  fi
  # Unset rather than 1: the profile's default (on for dev and test), as a
  # plain `cargo` in the same checkout gets, so both share one build.
  if [ "$incremental" = 1 ]; then unset CARGO_INCREMENTAL; else export CARGO_INCREMENTAL=0; fi
  # In a slot, link with mold where it's installed (`brew install mold`):
  # measured 20-45% faster links of a test binary (docs/performance.md,
  # "Test cycle"). Only there: changing rustflags rebuilds everything once,
  # which a running worktree shouldn't pay. AGENT_NO_MOLD=1 turns it off.
  case "$here" in
    */.claude/slots/*)
      if [ -z "${AGENT_NO_MOLD:-}" ] && [ "$(uname -s)" = Linux ] && command -v ld.mold >/dev/null 2>&1; then
        export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Clink-arg=-fuse-ld=mold"
      fi ;;
  esac
  "$here/sccache-heal.sh"
  # The server must not inherit the agents' jobserver: it uses its own.
  env -u MAKEFLAGS -u CARGO_MAKEFLAGS -u MFLAGS "$bin" --start-server >/dev/null 2>&1
}

# Run a command in agents.slice (falls back to running it directly).
agent_run() {
  if [ -n "$AGENT_NO_SLICE" ] || ! command -v systemd-run >/dev/null 2>&1 \
     || ! systemctl --user show -p Version --value >/dev/null 2>&1; then
    "$@"; return
  fi
  # Slice limits are runtime-only (gone at reboot); setting them is idempotent.
  if [ "$(systemctl --user show agents.slice -p CPUWeight --value 2>/dev/null)" != 50 ]; then
    systemctl --user set-property --runtime agents.slice MemoryHigh=24G CPUWeight=50 2>/dev/null
  fi
  systemd-run --user --scope --quiet --slice=agents.slice -- "$@"
}

_agent_sccache_init
_agent_env_init || echo "agent-env: could not set up the jobserver; builds run unthrottled" >&2
