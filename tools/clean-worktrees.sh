#!/usr/bin/env bash
# Remove the build folders (target/) of agent worktrees whose branch is
# merged into main and that no running process is using. Code and branches
# are kept; only regenerable build output goes. Usage: clean-worktrees.sh [repo]
# The build slots (.claude/slots/*, tools/agent-slot.sh) keep their target/
# on purpose (it's what makes them fast): this only trims the free ones
# past AGENT_SLOT_MAX_GB (60 GB) through `agent-slot.sh trim`, which also
# trims cargo's incremental folders in the main checkout's, the post-push
# slot's and the slots' target/ past CHIMERA_INCREMENTAL_MAX_GB (30 GB),
# oldest first, skipping a target a running cargo holds.
set -u
repo="${1:-$(git rev-parse --show-toplevel)}"
freed=0
for w in "$repo"/.claude/worktrees/*/; do
  [ -d "$w/target" ] || continue
  head=$(git -C "$w" rev-parse HEAD 2>/dev/null) || continue
  git -C "$repo" merge-base --is-ancestor "$head" main 2>/dev/null || continue
  # Skip worktrees with a process working inside them (an agent still busy).
  if pgrep -f "$(basename "$w")" >/dev/null 2>&1; then continue; fi
  if [ -n "$(git -C "$w" status --porcelain --untracked-files=no 2>/dev/null)" ]; then continue; fi
  kb=$(du -sk "$w/target" 2>/dev/null | cut -f1)
  rm -rf "$w/target" && freed=$((freed + ${kb:-0}))
done
echo "freed $((freed / 1024 / 1024)) GB"
if [ -d "$repo/.claude/slots" ]; then
  AGENT_SLOTS_DIR="$repo/.claude/slots" "$(dirname "$0")/agent-slot.sh" trim
else
  "$(dirname "$0")/agent-slot.sh" trim-incremental "$repo/target"
fi
