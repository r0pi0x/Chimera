#!/usr/bin/env bash
# Free disk space and the floor under which the caches stop writing: the
# shell side of chimera_assets' `disk` (same rule). Source it, then:
#   disk_check <path>   # prints "<free> GB free of <total> (floor <n>)"; returns 1 below the floor
# The floor is CHIMERA_DISK_FLOOR_GB (default 20, decimal GB) or 5% of the
# disk, whichever is smaller; 0 turns the check off. On a disk of an
# unreadable size it returns 0.
disk_check() {
  local path=${1:-.} line total_kb free_kb
  line=$(df -Pk -- "$path" 2>/dev/null | awk 'NR==2 {print $2, $4}') || return 0
  [ -n "$line" ] || return 0
  total_kb=${line% *}
  free_kb=${line#* }
  DISK_FREE_GB=$(awk -v k="$free_kb" 'BEGIN {printf "%.0f", k * 1024 / 1e9}')
  DISK_TOTAL_GB=$(awk -v k="$total_kb" 'BEGIN {printf "%.0f", k * 1024 / 1e9}')
  DISK_FLOOR_GB=$(awk -v t="$total_kb" -v f="${CHIMERA_DISK_FLOOR_GB:-20}" \
    'BEGIN {s = t * 1024 / 1e9 * 0.05; if (f + 0 < s) s = f + 0; printf "%.0f", s}')
  [ "${CHIMERA_DISK_FLOOR_GB:-20}" = 0 ] && return 0
  awk -v k="$free_kb" -v t="$total_kb" -v f="${CHIMERA_DISK_FLOOR_GB:-20}" \
    'BEGIN {s = t * 0.05; fk = f * 1e9 / 1024; if (fk < s) s = fk; exit !(k >= s)}'
}
