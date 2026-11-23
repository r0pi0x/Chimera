# Pass stamps, shared by .githooks/pre-commit and .githooks/pre-push
# (sourced, not run). A stamp records that one working tree passed some
# packages' checks and fast tests, so a later run needn't repeat them: on
# the same tree (the stamp's name is the tree's key, `tree_key`), or on a
# later tree for each package none of whose inputs changed since
# (`stamp_reuse`). Its lines:
#
#   when <UTC time> by <who> in <checkout>
#   tested <packages whose fast tier passed>       ("*": the workspace)
#   checked <packages whose tests type-checked>    (when more than tested)
#   tree <the tracked files' tree as tested>
#   env <hash of the toolchain, mode and CHIMERA_* settings>
#   u <size> <mtime> <path>                        (each untracked file)
#
# Stamps live in <git common dir>/test-stamps, the newest 100 kept;
# CHIMERA_COMMIT_STAMPS=0 ignores them.

stamps=$(git rev-parse --path-format=absolute --git-common-dir)/test-stamps
stamp_dependents=${CHIMERA_COMMIT_DEPENDENTS:-check}

# The working tree as tested, in three parts (`tree_state` sets them):
# st_tree, the tracked files' tree (from a scratch copy of the index, so
# neither the index nor the object store of untracked files is touched;
# the copy keeps the index's time, or git's check for files edited in the
# same second as the index was written ("racily clean") misses a
# same-size edit and the key is the old tree's); st_untracked, the
# untracked files' sizes, times and names; st_env, the toolchain, mode
# and CHIMERA_* settings. The test runner (cargo test or nextest) is not
# part of it: both run the same tests.
tree_state() {
    local idx
    idx=$(mktemp) || return 1
    cp -p "${GIT_INDEX_FILE:-$(git rev-parse --git-path index)}" "$idx" 2>/dev/null || { rm -f "$idx"; return 1; }
    st_tree=$(GIT_INDEX_FILE=$idx git add -u . 2>/dev/null && GIT_INDEX_FILE=$idx git write-tree) || { rm -f "$idx"; return 1; }
    rm -f "$idx"
    st_untracked=$(git ls-files --others --exclude-standard -z | xargs -0 -r stat -c '%s %Y %n' 2>/dev/null | LC_ALL=C sort -k3)
    st_env=$( {
        echo "features dev; fast tier (--skip heavy::); dependents $stamp_dependents"
        rustc -vV 2>/dev/null
        cargo -V 2>/dev/null
        env | grep -E '^(CHIMERA|MASHUP)_' | grep -vE '^(CHIMERA|MASHUP)_(COMMIT_TESTS|COMMIT_STAMPS|COMMIT_RUNNER|HOOK_FAILFAST)=' | sort
    } | git hash-object --stdin)
}

# The key of the state `tree_state` set: its three parts hashed.
state_key() { printf 'tree %s\n%s\nenv %s\n' "$st_tree" "$st_untracked" "$st_env" | git hash-object --stdin; }

# The working tree's key (`$(tree_key)` runs in a subshell: call
# `tree_state` first where the parts are needed after).
tree_key() { tree_state && state_key; }

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
    tree_state && [ "$(state_key)" = "$key" ] || { echo "$who: the working tree changed while testing; no stamp." >&2; return 0; }
    mkdir -p "$stamps" || return 0
    tested=$( { stamp_tested "$key"; printf '%s\n' "$3"; } | sed '/^$/d' | sort -u)
    checked=$( { stamp_checked "$key"; printf '%s\n' "$4"; } | sed '/^$/d' | sort -u | comm -23 - <(printf '%s\n' "$tested"))
    {
        echo "when $(date -u +%Y-%m-%dT%H:%M:%SZ) by $who in $(basename "$PWD")"
        echo "tested $(printf '%s\n' "$tested" | tr '\n' ' ')"
        if [ -n "$checked" ]; then echo "checked $(printf '%s\n' "$checked" | tr '\n' ' ')"; fi
        echo "tree $st_tree"
        echo "env $st_env"
        if [ -n "$st_untracked" ]; then printf '%s\n' "$st_untracked" | sed 's/^/u /'; fi
    } >"$stamps/$key.tmp" && mv -f "$stamps/$key.tmp" "$stamps/$key"
    rm -f "$stamps/$key.tmp"
    # shellcheck disable=SC2012
    ls -t "$stamps" 2>/dev/null | tail -n +101 | while read -r f; do rm -f "$stamps/$f"; done
}

# The untracked files added, removed or changed (size or time) between
# stamp $1 and now (after `tree_state`), one path per line.
stamp_untracked_changes() {
    LC_ALL=C comm -3 <(sed -n 's/^u //p' "$stamps/$1" 2>/dev/null | LC_ALL=C sort) <(printf '%s\n' "$st_untracked" | sed '/^$/d' | LC_ALL=C sort) \
        | sed 's/^\t//' | cut -d' ' -f3- | LC_ALL=C sort -u
}

# What changed between stamp $1's tree and the current one (after
# `tree_state`): the tracked paths that differ, then the untracked
# files' changes, one per line. Fails when the stamp has no tree (an
# older stamp) or git no longer has it.
stamp_changes() {
    local t
    t=$(sed -n 's/^tree //p' "$stamps/$1" 2>/dev/null)
    [ -n "$t" ] || return 1
    git diff --name-only --no-renames "$t" "$st_tree" 2>/dev/null || return 1
    stamp_untracked_changes "$1"
}

# Per-package reuse (after `tree_state`): of the newest 30 stamps made
# with the same toolchain, mode and settings (st_env), the 3 whose trees
# differ least from this one; for each, `affected` (the caller's
# `affected_cmd`, given `--trees <its tree> <this tree> [--files
# <untracked changes>] --format cargo --quiet`) selects every package
# whose inputs changed since: its own files or those of anything it
# depends on, the lockfile's resolved dependencies, the workspace's
# settings and hooks (all of them), as for a commit. The stamp's other
# packages passed on inputs identical to this tree's, so they still pass.
# Sets reuse_tested and reuse_checked (pkgs lists, the union over the
# stamps; "*" only from a stamp of an identical tree) and reuse_from (the
# stamps used). $1, when given, is what this run needs (a pkgs list): a
# stamp with none of it is skipped (each lookup costs a cargo run, ~2 s).
# Never fails: no stamp, no reuse.
stamp_reuse() {
    local f n changes sel invalid tested checked best
    local -a extra
    reuse_tested= reuse_checked= reuse_from=
    [ -d "$stamps" ] || return 0
    # shellcheck disable=SC2012
    best=$(for f in $(ls -t "$stamps" 2>/dev/null | grep -v '\.tmp$' | head -n 30); do
        grep -qx "env $st_env" "$stamps/$f" 2>/dev/null || continue
        changes=$(stamp_changes "$f") || continue
        echo "$(printf '%s' "$changes" | grep -c .) $f"
    done | sort -n -s | head -n 3)
    while read -r n f; do
        [ -n "$f" ] || continue
        tested=$(stamp_tested "$f")
        checked=$(stamp_checked "$f")
        # Nothing this run needs (pkgs list $1, when given): no lookup.
        if [ -n "${1:-}" ] && [ -z "$(comm -12 <(printf '%s\n' "$checked") <(printf '%s\n' "$1" | sort -u) | sed '/^$/d')" ]; then continue; fi
        invalid=
        if [ "$n" != 0 ]; then
            # A pass of the whole workspace names no packages to keep.
            grep -qx '\*' <<<"$tested" && continue
            mapfile -t extra < <(stamp_untracked_changes "$f")
            [ "${#extra[@]}" -gt 0 ] && extra=(--files "${extra[@]}")
            sel=$(affected_cmd --trees "$(sed -n 's/^tree //p' "$stamps/$f")" "$st_tree" "${extra[@]}" --format cargo --quiet 2>/dev/null) || continue
            invalid=$(pkgs "$sel")
            grep -qx '\*' <<<"$invalid" && continue
        fi
        tested=$(comm -23 <(printf '%s\n' "$tested") <(printf '%s\n' "$invalid") | sed '/^$/d')
        checked=$(comm -23 <(printf '%s\n' "$checked") <(printf '%s\n' "$invalid") | sed '/^$/d')
        [ -n "$tested$checked" ] || continue
        reuse_tested=$(printf '%s\n%s\n' "$reuse_tested" "$tested" | sed '/^$/d' | sort -u)
        reuse_checked=$(printf '%s\n%s\n' "$reuse_checked" "$checked" | sed '/^$/d' | sort -u)
        reuse_from="$reuse_from${reuse_from:+ }$f"
    done <<<"$best"
    return 0
}

# Cargo selection $1 (one argument per line) narrowed to the packages of
# pkgs list $2: their `-p` pairs, the `--features` naming them (or no
# package), any other option as it was.
keep_pkgs() {
    awk -v keep="$(printf '%s\n' "$2" | tr '\n' ' ')" '
        BEGIN { n = split(keep, a, " "); for (i = 1; i <= n; i++) k[a[i]] = 1 }
        p { if ($0 in k) printf "-p\n%s\n", $0; p = 0; next }
        /^-p$/ { p = 1; next }
        f { out = ""; m = split($0, fs, ",")
            for (i = 1; i <= m; i++) { split(fs[i], q, "/"); if (!index(fs[i], "/") || (q[1] in k)) out = out (out == "" ? "" : ",") fs[i] }
            if (out != "") printf "--features\n%s\n", out; f = 0; next }
        /^--features$/ { f = 1; next }
        { print }' <<<"$1"
}
