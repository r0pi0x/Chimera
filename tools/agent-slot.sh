#!/usr/bin/env bash
# Persistent build slots for agents: git worktrees at fixed paths
# (<repo>/.claude/slots/slot1..N) whose target/ stays warm from one agent
# to the next. Cargo fingerprints workspace crates by their absolute path,
# so a fresh worktree (or a shared CARGO_TARGET_DIR between different
# paths) rebuilds every workspace crate; a slot at the same path rebuilds
# only what the new branch's files change. In a slot, tools/agent-env.sh
# leaves cargo's incremental builds on (sccache still caches the
# registry crates). docs/OBSERVABILITY.md 3e has the numbers.
#
#   tools/agent-slot.sh acquire <branch> [base]   # prints the slot's path
#   tools/agent-slot.sh release [slot]            # the slot you're in by default
#   tools/agent-slot.sh status
#   tools/agent-slot.sh trim [slot...]            # shrink target/ past the limit
#   tools/agent-slot.sh trim-incremental [target...]   # trim target/*/incremental
#
# acquire takes a free slot (flock), creates its worktree the first time,
# and checks out <branch>: reset to [base] when given (`git checkout -B`),
# else the existing branch, else a new branch from main. What a previous
# holder left uncommitted is saved as a patch next to the slots
# (<slots>/slotN-discarded-<time>.patch), then the slot is reset and
# cleaned (untracked files go; ignored ones, target/, stay). release
# detaches HEAD (the branch can then be checked out or deleted elsewhere)
# and frees the slot. A slot is held until released: `status` shows who
# holds which, `release slotN` frees one whose agent is gone.
# Settings: AGENT_SLOTS (number of slots, default 3), AGENT_SLOT_MAX_GB
# (trim target/ past this, default 60: a whole-workspace test build is
# ~30 GB with the dev profile's split debug info, ~50 GB before it;
# acquire trims the slot it takes, its incremental caches first),
# AGENT_SLOTS_DIR (where the slots live).
#
# trim-incremental: cargo's incremental caches (target/<profile>/incremental,
# also under a target triple) grow without bound (the main checkout's was
# 206 GB on 2026-10-31). It trims them in the main checkout's target/, the
# post-push slot's, and every slot's (or the target folders named), oldest
# session first, down to 3/4 of CHIMERA_INCREMENTAL_MAX_GB (default 30)
# when they are past it. A target a running cargo holds (a flock on its
# .cargo-lock files) is left alone. `trim` does it too.
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)
repo=$(dirname "$common")
slots_dir=${AGENT_SLOTS_DIR:-$repo/.claude/slots}
count=${AGENT_SLOTS:-3}
max_gb=${AGENT_SLOT_MAX_GB:-60}
mkdir -p "$slots_dir"

say() { echo "agent-slot: $*" >&2; }

target_gb() { # <slot>: its target/ in whole GB
  if [ -d "$1/target" ]; then du -s --block-size=1G "$1/target" 2>/dev/null | cut -f1; else echo 0; fi
}

# Shrink <slot>'s target/ when it's past the limit: cargo-sweep drops the
# oldest artifacts down to 3/4 of it; without cargo-sweep the incremental
# caches go, then the whole folder if that wasn't enough.
trim_slot() {
  local slot=$1 gb
  # The incremental caches first, by their own limit: cargo-sweep never
  # removes them, and in a used slot they are most of target/ (60 of 67 GB
  # in one on 2026-11-10), so sweeping by size alone took every built
  # crate and test binary (27 of 28 GB of deps/ in another) and left the
  # caches: the next holder built from cold, and was over the limit again.
  trim_incremental "$slot/target"
  gb=$(target_gb "$slot")
  [ "$gb" -gt "$max_gb" ] || return 0
  say "$(basename "$slot")/target is $gb GB (limit $max_gb): trimming"
  if command -v cargo-sweep >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/cargo-sweep" ]; then
    (cd "$slot" && PATH="$HOME/.cargo/bin:$PATH" cargo sweep --maxsize "$((max_gb * 3 / 4))GB" . >/dev/null 2>&1) || true
  else
    find "$slot/target" -maxdepth 3 -type d -name incremental -prune -exec rm -rf {} + 2>/dev/null || true
  fi
  gb=$(target_gb "$slot")
  if [ "$gb" -gt "$max_gb" ]; then
    say "still $gb GB: removing $(basename "$slot")/target"
    rm -rf "$slot/target"
  fi
}

inc_max_gb=${CHIMERA_INCREMENTAL_MAX_GB:-30}

# Whether a cargo is building in <target>: it holds a flock on the
# .cargo-lock beside its profile folder. Without flock(1), assume so.
target_in_use() {
  local f
  command -v flock >/dev/null 2>&1 || return 0
  while IFS= read -r f; do
    flock -n "$f" true 2>/dev/null || return 0
  done < <(find "$1" -maxdepth 3 -name .cargo-lock 2>/dev/null)
  return 1
}

# Trim <target>'s incremental folders past the limit, oldest session
# folders (incremental/<crate>-<hash>/s-*) first, down to 3/4 of it.
trim_incremental() {
  local target=$1 limit_kb goal_kb total_kb=0 kb dir
  local -a incs=()
  [ -d "$target" ] || return 0
  limit_kb=$(awk -v g="$inc_max_gb" 'BEGIN {printf "%d", g * 1048576}')
  goal_kb=$((limit_kb * 3 / 4))
  while IFS= read -r -d '' dir; do incs+=("$dir"); done \
    < <(find "$target" -maxdepth 3 -type d -name incremental -print0 2>/dev/null)
  [ ${#incs[@]} -gt 0 ] || return 0
  total_kb=$(du -sk -- "${incs[@]}" 2>/dev/null | awk '{s += $1} END {print s + 0}')
  [ "$total_kb" -gt "$limit_kb" ] || return 0
  if target_in_use "$target"; then
    say "$target: incremental is $((total_kb / 1048576)) GB (limit $inc_max_gb) but a cargo is using it: left alone"
    return 0
  fi
  say "$target: incremental is $((total_kb / 1048576)) GB (limit $inc_max_gb): trimming oldest first"
  while IFS= read -r -d '' dir; do
    [ "$total_kb" -gt "$goal_kb" ] || break
    dir=${dir#* }
    kb=$(du -sk -- "$dir" 2>/dev/null | cut -f1)
    rm -rf -- "$dir" && total_kb=$((total_kb - ${kb:-0}))
  done < <(find "${incs[@]}" -mindepth 2 -maxdepth 2 -type d -printf '%T@ %p\0' 2>/dev/null | sort -z -n)
  find "${incs[@]}" -mindepth 1 -maxdepth 1 -type d -empty -delete 2>/dev/null || true
  say "$target: incremental now $((total_kb / 1048576)) GB"
}

# The main checkout's target/, the post-push slot's, each slot's (or the
# targets named).
trim_incrementals() {
  local t
  if [ $# -eq 0 ]; then
    set -- "$repo/target" "$slots_dir/post-push/target" "$slots_dir"/slot*/target
  fi
  for t in "$@"; do trim_incremental "$t"; done
}

# The slot holding the current directory, or the one named (slotN or a path).
slot_of() {
  local s=${1:-}
  if [ -z "$s" ]; then
    s=$(git rev-parse --show-toplevel 2>/dev/null) || { say "not in a slot: name one (slot1..slot$count)"; exit 2; }
  fi
  case "$s" in
    /*) ;;
    *) s="$slots_dir/$s" ;;
  esac
  s=${s%/}
  case "$s" in
    "$slots_dir"/*) echo "$s" ;;
    *) say "$s isn't a slot (they're under $slots_dir)"; exit 2 ;;
  esac
}

acquire() {
  local branch=${1:-} base=${2:-} slot="" name i
  [ -n "$branch" ] || { say "usage: agent-slot.sh acquire <branch> [base]"; exit 2; }
  # Pick and mark a free slot under the lock; prefer the one that last
  # held this branch (its build is the closest).
  {
    flock 9
    for i in $(seq 1 "$count"); do
      [ -e "$slots_dir/slot$i.lock" ] && continue
      if [ -z "$slot" ] || [ "$(cat "$slots_dir/slot$i.last" 2>/dev/null)" = "$branch" ]; then
        slot="$slots_dir/slot$i"
      fi
    done
    if [ -z "$slot" ]; then
      say "all $count slots are held (AGENT_SLOTS raises the count):"
      status >&2
      exit 1
    fi
    printf '%s %s\n' "$branch" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$slot.lock"
  } 9>"$slots_dir/.lock"
  name=$(basename "$slot")
  # From here on, a failure frees the slot again.
  trap 'rm -f "$slot.lock"; say "could not prepare $name; freed it"' ERR
  if [ ! -e "$slot/.git" ]; then
    say "creating $name (its first build is a cold one)"
    git -C "$repo" worktree add --quiet --detach "$slot" "${base:-main}"
  fi
  if [ -n "$(git -C "$slot" status --porcelain 2>/dev/null)" ]; then
    local patch
    patch="$slots_dir/$name-discarded-$(date -u +%Y%m%dT%H%M%SZ).patch"
    { git -C "$slot" status --short; git -C "$slot" diff HEAD; } >"$patch" || true
    say "$name had uncommitted changes: saved to $patch, discarding them"
  fi
  git -C "$slot" reset --quiet --hard
  git -C "$slot" clean -fdq
  if [ -n "$base" ]; then
    git -C "$slot" checkout --quiet -B "$branch" "$base"
  elif git -C "$slot" show-ref --verify --quiet "refs/heads/$branch"; then
    git -C "$slot" checkout --quiet "$branch"
  else
    git -C "$slot" checkout --quiet -B "$branch" main
  fi
  trap - ERR
  echo "$branch" >"$slot.last"
  trim_slot "$slot"
  say "$name holds $branch at $(git -C "$slot" log -1 --format='%h %s' | cut -c1-70)"
  say "work there: cd $slot; source tools/agent-env.sh; agent_run cargo ...; release with tools/agent-slot.sh release"
  echo "$slot"
}

release() {
  local slot name
  slot=$(slot_of "${1:-}")
  name=$(basename "$slot")
  if [ -e "$slot/.git" ]; then
    if [ -n "$(git -C "$slot" status --porcelain 2>/dev/null)" ]; then
      say "$name has uncommitted changes; the next acquire saves them as a patch and discards them"
    else
      git -C "$slot" checkout --quiet --detach
    fi
  fi
  rm -f "$slot.lock"
  say "released $name"
}

status() {
  local i slot held
  for i in $(seq 1 "$count"); do
    slot="$slots_dir/slot$i"
    if [ -e "$slot.lock" ]; then held="held: $(cat "$slot.lock")"; else held="free"; fi
    if [ -e "$slot/.git" ]; then
      printf 'slot%s  %-40s  %s  target %s GB\n' "$i" "$held" \
        "$(git -C "$slot" log -1 --format=%h 2>/dev/null)" "$(target_gb "$slot")"
    else
      printf 'slot%s  %-40s  (not created yet)\n' "$i" "$held"
    fi
  done
}

trim() {
  local s slot
  if [ $# -eq 0 ]; then
    # The free slots only: a held slot may be building.
    for s in "$slots_dir"/slot*/; do
      s=${s%/}
      [ -e "$s.lock" ] || trim_slot "$s"
    done
    trim_incrementals
    return
  fi
  for s in "$@"; do
    slot=$(slot_of "$s")
    trim_slot "$slot"
  done
}

cmd=${1:-}
[ $# -gt 0 ] && shift
case "$cmd" in
  acquire) acquire "$@" ;;
  release) release "$@" ;;
  status) status ;;
  trim) trim "$@" ;;
  trim-incremental) trim_incrementals "$@" ;;
  *) sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
