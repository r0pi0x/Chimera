# Pass stamps, shared by .githooks/pre-commit and .githooks/pre-push
# (sourced, not run). A stamp records that one exact working tree passed
# some packages' checks and fast tests, so a later run on the same tree
# needn't repeat them. Its name is the tree's key (`tree_key`), its lines:
#
#   when <UTC time> by <who> in <checkout>
#   tested <packages whose fast tier passed>       ("*": the workspace)
#   checked <packages whose tests type-checked>    (when more than tested)
#
# Stamps live in <git common dir>/test-stamps, the newest 100 kept;
# CHIMERA_COMMIT_STAMPS=0 ignores them.

stamps=$(git rev-parse --path-format=absolute --git-common-dir)/test-stamps
stamp_dependents=${CHIMERA_COMMIT_DEPENDENTS:-check}

# The working tree as tested: the tracked files' tree (from a scratch copy
# of the index, so neither the index nor the object store of untracked
# files is touched; the copy keeps the index's time, or git's check for
# files edited in the same second as the index was written ("racily
# clean") misses a same-size edit and the key is the old tree's), the untracked files' names, sizes and times, the
# toolchain and the CHIMERA_* settings. The test runner (cargo test or
# nextest) is not part of it: both run the same tests.
tree_key() {
    local idx tree
    idx=$(mktemp) || return 1
    cp -p "${GIT_INDEX_FILE:-$(git rev-parse --git-path index)}" "$idx" 2>/dev/null || { rm -f "$idx"; return 1; }
    tree=$(GIT_INDEX_FILE=$idx git add -u . 2>/dev/null && GIT_INDEX_FILE=$idx git write-tree) || { rm -f "$idx"; return 1; }
    rm -f "$idx"
    {
        echo "tree $tree"
        git ls-files --others --exclude-standard -z | xargs -0 -r stat -c '%n %s %Y' 2>/dev/null | sort
        echo "features dev; fast tier (--skip heavy::); dependents $stamp_dependents"
        rustc -vV 2>/dev/null
        cargo -V 2>/dev/null
        env | grep -E '^(CHIMERA|MASHUP)_' | grep -vE '^(CHIMERA|MASHUP)_(COMMIT_TESTS|COMMIT_STAMPS|COMMIT_RUNNER)=' | sort
    } | git hash-object --stdin
}

# The packages a cargo selection names, one per line (`--workspace`: "*").
pkgs() { printf '%s\n' "$1" | awk '/^--workspace$/ { print "*"; next } p { print; p = 0; next } /^-p$/ { p = 1 }' | sort -u; }

# Is every package of $1 (a pkgs list) in $2? "*" covers all.
covered() { grep -qx '\*' <<<"$2" && return 0; [ -z "$(comm -23 <(printf '%s\n' "$1") <(printf '%s\n' "$2"))" ]; }

# The packages stamp $1 tested, and those it tested or checked.
stamp_tested() { sed -n 's/^tested //p' "$stamps/$1" 2>/dev/null | tr ' ' '\n' | sed '/^$/d' | sort -u; }
stamp_checked() { { stamp_tested "$1"; sed -n 's/^checked //p' "$stamps/$1" 2>/dev/null | tr ' ' '\n'; } | sed '/^$/d' | sort -u; }

# Record a pass of key $1 by $2: tested packages $3, checked $4 (pkgs
# lists), merged with what the stamp already had. Only when the tree is
# still the one tested.
stamp_record() {
    local key=$1 who=$2 tested checked
    [ -n "$key" ] || return 0
    [ "$(tree_key)" = "$key" ] || { echo "$who: the working tree changed while testing; no stamp." >&2; return 0; }
    mkdir -p "$stamps" || return 0
    tested=$( { stamp_tested "$key"; printf '%s\n' "$3"; } | sed '/^$/d' | sort -u)
    checked=$( { stamp_checked "$key"; printf '%s\n' "$4"; } | sed '/^$/d' | sort -u | comm -23 - <(printf '%s\n' "$tested"))
    {
        echo "when $(date -u +%Y-%m-%dT%H:%M:%SZ) by $who in $(basename "$PWD")"
        echo "tested $(printf '%s\n' "$tested" | tr '\n' ' ')"
        [ -n "$checked" ] && echo "checked $(printf '%s\n' "$checked" | tr '\n' ' ')"
    } >"$stamps/$key.tmp" && mv -f "$stamps/$key.tmp" "$stamps/$key"
    # shellcheck disable=SC2012
    ls -t "$stamps" 2>/dev/null | tail -n +101 | while read -r f; do rm -f "$stamps/$f"; done
}
