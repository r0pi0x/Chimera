#!/usr/bin/env bash
# Test what was pushed, after the push (push first, test after: the
# pre-push hook keeps only the fast checks). Run it right after each push:
#
#   tools/post-push-tests.sh [base]
#
# Tests the remote's main (CHIMERA_POST_PUSH_REF: another ref) in its own
# warm checkout, .claude/slots/post-push (a git worktree at a fixed path,
# created the first time; its target/ is kept), so main's working tree
# isn't touched. What it runs, as the pre-push hook did before:
#  * the build without --features dev (with the debugging tools), the
#    dedicated servers' builds without the presentation, and the
#    workspace-hack against the dependencies (where cargo-hakari is);
#  * the tests of every package the pushed changes can affect, heavy ones
#    included (`affected --range <base>..<pushed>`; nextest when installed).
#    [base] defaults to the last commit this script passed, else the
#    pushed commit's first parent. CHIMERA_PUSH_TESTS=all runs the full
#    suite, =0 skips the tests (keeping the build checks). The tests that
#    need the games' installs use the main checkout's local config
#    (CHIMERA_CONFIG set to its chimera.local.toml or mashup.local.toml
#    when there is one and the caller set none; the file isn't read here).
# Writes .claude/post-push/<commit>.log (also last.log) and a one-line
# summary to .claude/post-push/last.txt (appended to history.txt), and
# exits non-zero on a failure. One run at a time (a second waits).
set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)
repo=$(dirname "$common")
dir=${CHIMERA_POST_PUSH_DIR:-$repo/.claude/slots/post-push}
out=$repo/.claude/post-push
remote=${CHIMERA_POST_PUSH_REMOTE:-origin}
ref=${CHIMERA_POST_PUSH_REF:-$remote/main}
mode=${CHIMERA_PUSH_TESTS:-${MASHUP_PUSH_TESTS:-affected}}
mkdir -p "$out"
exec 8>"$out/.lock"
flock -n 8 || { echo "post-push: another run is going; waiting for it" >&2; flock 8; }

git -C "$repo" fetch --quiet --no-tags "$remote" || echo "post-push: couldn't fetch $remote; testing what's here" >&2
sha=$(git -C "$repo" rev-parse --verify --quiet "$ref^{commit}") || { echo "post-push: no commit $ref" >&2; exit 2; }
base=${1:-$(cat "$out/last-pass" 2>/dev/null)}
[ -n "$base" ] || base=$(git -C "$repo" rev-parse --verify --quiet "$sha^1") || base=
short=$(git -C "$repo" rev-parse --short "$sha")
log=$out/$sha.log
started=$(date +%s)

if [ ! -e "$dir/.git" ]; then
    echo "post-push: creating its checkout at $dir (the first run builds from cold)" >&2
    git -C "$repo" worktree add --quiet --detach "$dir" "$sha" || exit 2
fi
git -C "$dir" reset --quiet --hard && git -C "$dir" clean -fdq && git -C "$dir" checkout --quiet --detach "$sha" || exit 2
cd "$dir" || exit 2
# shellcheck source=/dev/null
set +u  # agent-env.sh and its agent_run read unset variables
. tools/agent-env.sh
# The heavy tests that need the games' installs find them through the
# local config, which lives in the main checkout, not this one (git-ignored):
# point CHIMERA_CONFIG at it (never read here) unless the caller set one,
# so those tests run instead of skipping with a note.
if [ -z "${CHIMERA_CONFIG:-}" ] && [ -z "${MASHUP_CONFIG:-}" ]; then
    for f in chimera.local.toml mashup.local.toml; do
        if [ -f "$repo/$f" ]; then
            export CHIMERA_CONFIG="$repo/$f"
            echo "post-push: the installs from the main checkout's $f" >&2
            break
        fi
    done
fi
ln -sf "$sha.log" "$out/last.log"
: >"$log"

failed=
step() { # <what> <command...>: run, log, note a failure
    local what=$1; shift
    echo "post-push: $what..." >&2
    echo "=== $what: $*" >>"$log"
    if ! agent_run "$@" >>"$log" 2>&1; then
        echo "post-push: FAILED: $what" >&2
        failed="${failed:+$failed; }$what"
        return 1
    fi
}

# What the tests build with (--features dev) isn't what playtest and
# release builds use: check those too, with the binaries behind features
# (the debugging tools: `tools`).
step "the build without --features dev" cargo check --workspace --lib --bins --features tools --quiet
# The dedicated servers as released: without the presentation
# (apps/cs_source_server/Cargo.toml, apps/combat_arms_server/Cargo.toml).
step "the dedicated servers' builds" cargo check -p cs_source_server -p combat_arms_server --quiet
if cargo hakari --version >/dev/null 2>&1; then
    step "the workspace-hack (cargo hakari generate --diff)" cargo hakari generate --diff --quiet
fi

tests="skipped"
if [ "$mode" != 0 ]; then
    if cargo nextest --version >/dev/null 2>&1; then
        format=nextest
        runner=(cargo nextest run --features dev --no-fail-fast --color never)
    else
        format=cargo
        runner=(cargo test --features dev --quiet --no-fail-fast)
    fi
    if [ "$mode" = all ] || [ -z "$base" ]; then
        args=--workspace
        echo "post-push: the full suite (CHIMERA_PUSH_TESTS=$mode, base ${base:-none})" >&2
    elif ! args=$(agent_run cargo run --quiet -p repo_checks --bin affected -- --range "$base..$sha" --remote "$remote" --format $format 2>"$out/affected.txt"); then
        cat "$out/affected.txt" >&2
        echo "post-push: couldn't select the affected tests; running the full suite." >&2
        args=--workspace
    else
        sed 's/^/post-push: /' "$out/affected.txt" >&2
    fi
    # One argument per line (the selection prints them so; the filter has
    # spaces).
    mapfile -t sel <<<"$args"
    tests="${sel[*]}"
    step "the tests ($(git -C "$repo" rev-parse --short "$base" 2>/dev/null || echo none)..$short)" "${runner[@]}" "${sel[@]}"
fi

# Keep this checkout's build folder in bounds (the oldest artifacts go).
if command -v cargo-sweep >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/cargo-sweep" ]; then
    PATH="$HOME/.cargo/bin:$PATH" cargo sweep --maxsize "${AGENT_SLOT_MAX_GB:-60}GB" . >/dev/null 2>&1 || true
fi

mins=$(( ($(date +%s) - started + 59) / 60 ))
if [ -z "$failed" ]; then
    line="PASS $short $(date -u +%Y-%m-%dT%H:%MZ) ${mins}m tests: ${tests:0:200}"
    echo "$sha" >"$out/last-pass"
else
    line="FAIL $short $(date -u +%Y-%m-%dT%H:%MZ) ${mins}m: $failed (log: $log)"
    grep -E "panicked|FAILED|FAIL \[|^error" "$log" | head -20 >&2
fi
echo "$line" >"$out/last.txt"
echo "$line" >>"$out/history.txt"
echo "post-push: $line" >&2
[ -z "$failed" ]
