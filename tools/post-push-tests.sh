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
#    suite, =fast tests only what the pushed commits changed (base: the
#    pushed commit's first parent, never the last pass) and leaves out
#    the heavy tests, =0 skips the tests (keeping the build checks). The
#    mode can also come from the untracked file .claude/post-push/mode
#    (one word), which sets the default for every agent on this box. The test
#    binaries are linked without debug info (CARGO_PROFILE_TEST_STRIP=
#    debuginfo, set for the test step only; CHIMERA_POST_PUSH_STRIP=none
#    keeps it) to cut the bytes written: a failure's panic keeps its
#    message and file:line, but backtrace frames lose theirs and
#    RUST_BACKTRACE=1 prints none. To get lines back, rerun the test with
#    RUST_BACKTRACE=full (names) in a slot with `cargo test --profile dev`
#    (docs/performance.md, "Test binaries without debug info"). The tests that
#    need the games' installs use the main checkout's local config
#    (CHIMERA_CONFIG set to its chimera.local.toml or mashup.local.toml
#    when there is one and the caller set none; the file isn't read here).
# Don't wait for it. Agents start it in the background and finish:
#
#   tools/post-push-tests.sh --bg --note "<branch or task, one line>"
#
# --bg detaches the run (setsid; it outlives the shell that started it,
# its output in .claude/post-push/bg.out) and returns at once. --note
# (or CHIMERA_PUSH_NOTE) records who pushed what in pushes.txt, so a
# failure names the pushes it may come from. A FAIL is routed to the
# orchestrator, not to the pusher: the run writes a record,
# .claude/post-push/fails/<UTC time>-<commit>.md (the failing steps and
# tests, the commits since the last pass with the pushes' notes, the log,
# and a brief a fix agent can start from), and appends one line to
# .claude/post-push/inbox: `FAIL <commit> <time> <record>`; a later PASS
# after a FAIL appends `FIXED <commit> <time>`. Watch it with
# `tail -n0 -F .claude/post-push/inbox` (one Monitor for the whole
# session). CHIMERA_POST_PUSH_ON_FAIL=<command> also runs that command
# with the record's path (e.g. a desktop notification); nothing by default.
# CHIMERA_POST_PUSH_OUT moves .claude/post-push (the scripts' tests).
# Writes .claude/post-push/<commit>.<UTC time>.<pid>.log (also last.log; the
# newest CHIMERA_POST_PUSH_KEEP_LOGS, default 40, are kept) and
# a one-line summary to .claude/post-push/last.txt (appended to
# history.txt), and exits non-zero on a failure. One run at a time: a second
# waits, and only the newest waiter keeps waiting (older ones exit 0, the
# newest tests the latest main, from the last pass on). A run that holds the
# lock is never stopped. The lock and the queue live in the repository's
# common .claude/post-push whichever checkout's copy of this script started,
# and a run whose copy differs from the pushed one re-executes the pushed
# one (from its checkout) before testing. Nothing the run starts inherits
# the lock (a leaked `sleep infinity` holding it starved every run for six
# hours); a lock that no live run holds is moved aside after two polls. No
# verdict for CHIMERA_POST_PUSH_STALE_MIN (default 120) minutes while
# pushes wait puts one `STALE` line in the inbox. A commit with a result in
# history.txt isn't tested again (it exits 0 saying so) unless
# CHIMERA_POST_PUSH_AGAIN=1 or a [base] is given.
set -u
# Set by a run that re-executes the pushed copy of this script (it holds
# the lock on fd 8 already); never passed on to what this run starts.
locked=${CHIMERA_POST_PUSH_LOCKED:-}
reexec=${CHIMERA_POST_PUSH_REEXEC:-}
unset CHIMERA_POST_PUSH_LOCKED CHIMERA_POST_PUSH_REEXEC
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)
repo=$(dirname "$common")
dir=${CHIMERA_POST_PUSH_DIR:-$repo/.claude/slots/post-push}
out=${CHIMERA_POST_PUSH_OUT:-$repo/.claude/post-push}
remote=${CHIMERA_POST_PUSH_REMOTE:-origin}
ref=${CHIMERA_POST_PUSH_REF:-$remote/main}
mkdir -p "$out"
bg=0
note=${CHIMERA_PUSH_NOTE:-}
while [ $# -gt 0 ]; do
    case "$1" in
        --bg) bg=1; shift ;;
        --note) note=${2:-}; shift 2 ;;
        --note=*) note=${1#--note=}; shift ;;
        *) break ;;
    esac
done
# Who pushed what (the note), before anything can make this run step
# aside for a newer one: a failure lists the pushes in its range.
if [ -z "${CHIMERA_POST_PUSH_CHILD:-}" ]; then
    pushed=$(git -C "$here" rev-parse --verify --quiet HEAD) || pushed=
    branch=$(git -C "$here" rev-parse --abbrev-ref HEAD 2>/dev/null)
    printf '%s %s %s %s %s\n' "${pushed:-?}" "$(date -u +%Y-%m-%dT%H:%MZ)" "${branch:-?}" "$(basename "$(git -C "$here" rev-parse --show-toplevel 2>/dev/null)")" "${note:--}" >>"$out/pushes.txt"
    if [ "$(wc -l <"$out/pushes.txt")" -gt 400 ]; then
        tail -n 200 "$out/pushes.txt" >"$out/pushes.txt.$$" && mv -f "$out/pushes.txt.$$" "$out/pushes.txt"
    fi
fi
if [ "$bg" = 1 ]; then
    # Detached: its own session, so the starting shell (an agent's tool
    # call) can end; output appended to bg.out (cut to its last 2000 lines).
    if [ -f "$out/bg.out" ] && [ "$(wc -l <"$out/bg.out")" -gt 4000 ]; then
        tail -n 2000 "$out/bg.out" >"$out/bg.out.$$" && mv -f "$out/bg.out.$$" "$out/bg.out"
    fi
    CHIMERA_POST_PUSH_CHILD=1 setsid "$here/post-push-tests.sh" "$@" </dev/null >>"$out/bg.out" 2>&1 &
    echo "post-push: started in the background (pid $!); don't wait for it. The result: $out/last.txt; a FAIL also goes to $out/inbox (the orchestrator watches it)." >&2
    exit 0
fi
mode=${CHIMERA_PUSH_TESTS:-${MASHUP_PUSH_TESTS:-$(tr -d '[:space:]' <"$out/mode" 2>/dev/null)}}
mode=${mode:-affected}
# shellcheck source=/dev/null
. "$here/disk-free.sh"
if ! disk_check "$repo"; then
    echo "post-push: WARNING: low disk space: $DISK_FREE_GB GB free of $DISK_TOTAL_GB (floor $DISK_FLOOR_GB GB); the build may fail. tools/clean-worktrees.sh trims build folders" >&2
fi
# One run at a time, and one waiter: each run tests the newest pushed main,
# so a waiter that a newer one has joined behind has nothing left to do.
# Waiters register a rising token (nanoseconds) in .waiting (written whole by
# a rename) and poll the lock; one that sees a newer token exits. A run that
# holds the lock is never stopped. No pgrep of our own command line: those
# loops deadlocked before.
# The lock is an flock on fd 8, and every process that inherits fd 8 holds it
# too: the run closes it for all it starts (`8>&-` below), else a leftover
# descendant (a jobserver holder, a detached git gc) keeps the lock after the
# run and every later run waits forever. The run's pid and start time go to
# .running; a waiter that finds the lock held by no live run for two polls
# moves the lock file aside (under .lock.recover) and queues on a new one.
proc_field() { sed 's/.*) //' "/proc/$1/stat" 2>/dev/null | cut -d' ' -f"$2"; }
proc_start() { proc_field "$1" 20; }
runner_alive() {
    local pid st
    read -r pid st _ 2>/dev/null <"$out/.running" || return 1
    [ -n "$pid" ] && [ "$(proc_start "$pid")" = "$st" ]
}
lock_ino() { stat -Lc %i "/proc/$$/fd/8" 2>/dev/null; }
mine() { # this run or one of its subshells (they have fd 8 too)
    local p=$1
    while [ -n "$p" ] && [ "$p" -gt 1 ] 2>/dev/null; do
        [ "$p" = $$ ] && return 0
        p=$(proc_field "$p" 2)
    done
    return 1
}
lock_holders() { # the pids (and commands) other than this run's with the lock file open
    local f p
    for f in /proc/[0-9]*/fd/*; do
        [ "$(readlink "$f" 2>/dev/null)" = "$out/.lock" ] || continue
        p=${f#/proc/}; p=${p%%/*}
        mine "$p" && continue
        printf '%s (%s) ' "$p" "$(tr '\0' ' ' <"/proc/$p/cmdline" 2>/dev/null | cut -c1-60)"
    done
}
# No verdict for a long time while pushes wait: one STALE line in the inbox
# per last verdict, so starvation gets noticed.
stale_check() {
    local last t pushed limit=${CHIMERA_POST_PUSH_STALE_MIN:-120} who
    last=$(awk '$1 == "PASS" || $1 == "FAIL" { l = $0 } END { print l }' "$out/history.txt" 2>/dev/null)
    [ -n "$last" ] || return 0
    t=$(date -u -d "$(echo "$last" | cut -d' ' -f3)" +%s 2>/dev/null) || return 0
    [ $(( $(date +%s) - t )) -ge $(( limit * 60 )) ] || return 0
    pushed=$(tail -n 1 "$out/pushes.txt" 2>/dev/null | cut -d' ' -f2)
    pushed=$(date -u -d "$pushed" +%s 2>/dev/null) || return 0
    [ "$pushed" -gt "$t" ] || return 0
    [ "$(cat "$out/.stale" 2>/dev/null)" = "$last" ] && return 0
    echo "$last" >"$out/.stale"
    if runner_alive; then
        who="a run is going (pid, since): $(cut -d' ' -f1,3 "$out/.running")"
    else
        who="no live run holds the lock; it is open in: $(lock_holders)"
    fi
    echo "STALE $(date -u +%Y-%m-%dT%H:%MZ) no post-push verdict for over ${limit} min while pushes wait (last: $(echo "$last" | cut -d' ' -f1-3)); $who" >>"$out/inbox"
    echo "post-push: STALE: no verdict since $(echo "$last" | cut -d' ' -f3); $who" >&2
}
newer() { local w; w=$(cat "$out/.waiting" 2>/dev/null) && [ -n "$w" ] && [ "$w" -gt "$token" ] 2>/dev/null; }
stale_check
if [ "$locked" = 1 ] && [ -e "/proc/$$/fd/8" ]; then
    : # the run that re-executed this copy holds it
else
    exec 8>"$out/.lock"
    if ! flock -n 8; then
        token=$(date +%s%N)
        printf '%s\n' "$token" >"$out/.waiting.$$" && mv -f "$out/.waiting.$$" "$out/.waiting"
        echo "post-push: another run is going; waiting for it (a newer waiter replaces this one)" >&2
        dead=0
        until flock -w "${CHIMERA_POST_PUSH_POLL:-15}" 8; do
            newer && { echo "post-push: a newer run is waiting; it will test the latest main, so this one stops" >&2; exit 0; }
            stale_check
            # Another waiter moved a stale lock aside: queue on the new one.
            if [ "$(lock_ino)" != "$(stat -c %i "$out/.lock" 2>/dev/null)" ]; then
                exec 8>"$out/.lock"; dead=0; continue
            fi
            if runner_alive; then dead=0; continue; fi
            dead=$((dead + 1))
            [ "$dead" -ge 2 ] || continue
            (
                flock -n 7 || exit 0
                [ "$(lock_ino)" = "$(stat -c %i "$out/.lock" 2>/dev/null)" ] || exit 0
                runner_alive && exit 0
                held=$(lock_holders)
                mv -f "$out/.lock" "$out/.lock.stale" || exit 0
                echo "post-push: the lock was held by no live run but by: ${held:-?}; moved it aside (.lock.stale)" >&2
                echo "STALE $(date -u +%Y-%m-%dT%H:%MZ) the post-push lock was held by no live run but by: ${held:-?}(moved aside; kill that pid by hand)" >>"$out/inbox"
            ) 7>"$out/.lock.recover"
            exec 8>"$out/.lock"; dead=0
        done
        newer && { echo "post-push: a newer run is waiting; it will test the latest main, so this one stops" >&2; exit 0; }
    fi
fi
printf '%s %s %s\n' $$ "$(proc_start $$)" "$(date -u +%Y-%m-%dT%H:%MZ)" >"$out/.running"
trap '[ "$(cut -d" " -f1 "$out/.running" 2>/dev/null)" = $$ ] && rm -f "$out/.running"' EXIT

# From here on, nothing this run starts gets fd 8 (`8>&-`).
git -C "$repo" fetch --quiet --no-tags "$remote" 8>&- || echo "post-push: couldn't fetch $remote; testing what's here" >&2
sha=$(git -C "$repo" rev-parse --verify --quiet "$ref^{commit}" 8>&-) || { echo "post-push: no commit $ref" >&2; exit 2; }
# The pushed copy of this script tests the push: a run started from another
# copy (an older slot's) re-executes the pushed one, lock and all.
if [ -z "$reexec" ] && ! git -C "$repo" show "$sha:tools/post-push-tests.sh" 2>/dev/null 8>&- | cmp -s - "$here/post-push-tests.sh"; then
    if { [ -e "$dir/.git" ] || git -C "$repo" worktree add --quiet --detach "$dir" "$sha" 8>&-; } \
        && git -C "$dir" reset --quiet --hard 8>&- && git -C "$dir" checkout --quiet --detach "$sha" 8>&- \
        && [ -x "$dir/tools/post-push-tests.sh" ]; then
        echo "post-push: running the pushed copy of this script ($dir/tools/post-push-tests.sh)" >&2
        CHIMERA_POST_PUSH_LOCKED=1 CHIMERA_POST_PUSH_REEXEC=1 CHIMERA_POST_PUSH_CHILD=1 exec "$dir/tools/post-push-tests.sh" "$@"
    fi
fi

# The run proper, called with fd 8 closed: bash keeps its own copy
# (close-on-exec), so the lock stays held, and no process it starts holds it.
the_run() {
# Already tested (a result in history.txt): nothing to do, unless asked
# (CHIMERA_POST_PUSH_AGAIN=1, or a base given on the command line).
short=$(git -C "$repo" rev-parse --short "$sha")
if [ -z "${1:-}" ] && [ "${CHIMERA_POST_PUSH_AGAIN:-0}" = 0 ] \
    && prev=$(awk -v s="$sha" '($1 == "PASS" || $1 == "FAIL") && length($2) >= 7 && index(s, $2) == 1 { l = $0 } END { if (l == "") exit 1; print l }' "$out/history.txt" 2>/dev/null); then
    echo "post-push: $short already tested, nothing to do (CHIMERA_POST_PUSH_AGAIN=1 reruns): $prev" >&2
    exit 0
fi
base=${1:-$(cat "$out/last-pass" 2>/dev/null)}
[ "$mode" = fast ] && [ -z "${1:-}" ] && base=
[ -n "$base" ] || base=$(git -C "$repo" rev-parse --verify --quiet "$sha^1") || base=
# One log per run: two runs of a commit never share (nor truncate) one.
log=$out/$sha.$(date -u +%Y%m%dT%H%M%SZ).$$.log
started=$(date +%s)

if [ ! -e "$dir/.git" ]; then
    echo "post-push: creating its checkout at $dir (the first run builds from cold)" >&2
    git -C "$repo" worktree add --quiet --detach "$dir" "$sha" || exit 2
fi
# Its incremental caches grow without bound: trim them (CHIMERA_INCREMENTAL_MAX_GB;
# a target a running cargo holds is left alone) before building.
"$here/agent-slot.sh" trim-incremental "$dir/target" || true
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
ln -sf "$(basename "$log")" "$out/last.log"
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
    # Only this step: the checks above and `affected` build with the dev
    # profile, whose units the test profile's setting would split from.
    strip=CARGO_PROFILE_TEST_STRIP=${CHIMERA_POST_PUSH_STRIP:-debuginfo}
    if cargo nextest --version >/dev/null 2>&1; then
        format=nextest
        runner=(env "$strip" cargo nextest run --features dev --no-fail-fast --color never)
    else
        format=cargo
        runner=(env "$strip" cargo test --features dev --quiet --no-fail-fast)
    fi
    skip_heavy=
    [ "$mode" = fast ] && [ "$format" = nextest ] && skip_heavy=--skip-heavy
    if [ "$mode" = all ] || [ -z "$base" ]; then
        args=--workspace
        echo "post-push: the full suite (CHIMERA_PUSH_TESTS=$mode, base ${base:-none})" >&2
    elif ! args=$(agent_run cargo run --quiet -p repo_checks --bin affected -- --range "$base..$sha" --remote "$remote" --format $format ${skip_heavy:-} 2>"$out/affected.txt"); then
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
now=$(date -u +%Y-%m-%dT%H:%MZ)
prev_result=$(awk '$1 == "PASS" || $1 == "FAIL" { l = $1 } END { print l }' "$out/history.txt" 2>/dev/null)
if [ -z "$failed" ]; then
    line="PASS $short $now ${mins}m tests: ${tests:0:200}"
    echo "$sha" >"$out/last-pass"
    [ "$prev_result" = FAIL ] && echo "FIXED $short $now (the last run failed; this one passed)" >>"$out/inbox"
else
    line="FAIL $short $now ${mins}m: $failed (log: $log)"
    grep -E "panicked|FAILED|FAIL \[|^error" "$log" | head -20 >&2
    # The record a fix agent starts from.
    mkdir -p "$out/fails"
    record=$out/fails/$(date -u +%Y%m%dT%H%M%SZ)-$short.md
    {
        echo "# post-push FAIL at $short ($now)"
        echo
        echo "Failed: $failed"
        echo "Tested: $(git -C "$repo" rev-parse --short "$base" 2>/dev/null || echo none)..$short ($mins min, CHIMERA_PUSH_TESTS=$mode)"
        echo "Log: $log"
        echo
        echo "## Failing tests and errors"
        echo
        echo '```'
        grep -E '^ *FAIL \[|^test .* FAILED$|^error(\[|:)|panicked at' "$log" | sed 's/^ *//' | awk '!seen[$0]++' | head -40
        echo '```'
        echo
        echo "## Commits in the tested range (newest first)"
        echo
        if [ -n "$base" ]; then
            TZ=UTC git -C "$repo" log --format='- %h %ad %s' --date=format-local:%m-%dT%H:%MZ "$base..$sha" 2>/dev/null | head -40
            in_range=$(git -C "$repo" rev-list "$base..$sha" 2>/dev/null)
        else
            echo "(no base: the full suite ran)"
            in_range=
        fi
        echo
        echo "## Pushes in that range (pushes.txt: commit, time, branch, checkout, note)"
        echo
        if [ -n "$in_range" ]; then
            tail -n 200 "$out/pushes.txt" 2>/dev/null | while read -r c rest; do
                case "$in_range" in *"$c"*) echo "- ${c:0:9} $rest" ;; esac
            done
        fi
        echo
        echo "## Fix brief"
        echo
        echo "post-push tests failed on $remote/main at $short: $failed. Take a slot"
        echo "(\`tools/agent-slot.sh acquire fix-postpush-$short origin/main\`), reproduce the"
        echo "failing tests above with \`cargo nextest run --features dev -p <crate> -E 'test(<name>)'\`"
        echo "(or cargo test), find which commit of the range broke them (the pushes' notes"
        echo "say whose work it was), fix forward, push, and start"
        echo "\`tools/post-push-tests.sh --bg --note \"fix post-push $short\"\`. Don't wait for it."
    } >"$record" 2>/dev/null
    echo "FAIL $short $now $record" >>"$out/inbox"
    if [ -n "${CHIMERA_POST_PUSH_ON_FAIL:-}" ]; then
        sh -c "$CHIMERA_POST_PUSH_ON_FAIL \"\$1\"" _ "$record" >/dev/null 2>&1 || true
    fi
    # The newest 50 records.
    # shellcheck disable=SC2012
    ls -t "$out/fails" 2>/dev/null | tail -n +51 | while read -r f; do rm -f "$out/fails/$f"; done
fi
echo "$line" >"$out/last.txt"
echo "$line" >>"$out/history.txt"
echo "post-push: $line" >&2
# Keep the newest logs only (disk is tight); history.txt keeps every result.
# shellcheck disable=SC2012
ls -t "$out"/*.log 2>/dev/null | grep -v '/last\.log$' | tail -n +$(( ${CHIMERA_POST_PUSH_KEEP_LOGS:-40} + 1 )) | while read -r f; do rm -f "$f"; done
# A long inbox is rotated by rename (`tail -F` follows the new file and
# doesn't replay the old lines).
if [ "$(wc -l <"$out/inbox" 2>/dev/null || echo 0)" -gt 1000 ]; then
    mv -f "$out/inbox" "$out/inbox.old"
fi
[ -z "$failed" ]
}
the_run "$@" 8>&-
