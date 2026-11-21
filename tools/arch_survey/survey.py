#!/usr/bin/env python3
"""Measurements of the architecture review (docs/plans/active/architecture-review.md,
section 2), kept so the next pass compares like with like (T5).

    tools/arch_survey/survey.py lines      # per crate: source lines, test lines, tests
    tools/arch_survey/survey.py fns        # longest functions, most parameters
    tools/arch_survey/survey.py patterns   # pattern counts (unwrap, clone, Query tuples...)
    tools/arch_survey/survey.py structs    # widest Resource/Component/Message structs
    tools/arch_survey/survey.py closures   # each crate's dependency closure inside the workspace
    tools/arch_survey/survey.py commits [N]  # N (default 300) non-merge commits, by lowest crate touched
    tools/arch_survey/survey.py radius [DAYS]  # the last DAYS (default 14) days' commits: by kind, by lowest
                                             # layer, and how many crates' tests each can affect
                                             # (docs/plans/active/architecture-for-iteration.md)

Python 3 standard library only; run from anywhere in the repository. Counts
are approximate by design (brace matching on comment- and string-stripped
source, no parser): compare numbers made by the same script, not against a
compiler's. Inline `#[cfg(test)]` modules count as test lines, not source.
"""
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
SKIP = {"target", ".git", ".claude"}


def metadata():
    out = subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT, text=True)
    return json.loads(out)


def crates():
    """name -> crate directory (relative), longest directories first."""
    pkgs = {}
    for p in metadata()["packages"]:
        pkgs[p["name"]] = Path(p["manifest_path"]).parent.relative_to(ROOT).as_posix() or "."
    return pkgs


def crate_of(path, dirs):
    best = None
    for name, d in dirs.items():
        if d == "." and "/" not in path and best is None:
            best = (0, name)
        elif d != "." and path.startswith(d + "/") and (best is None or len(d) > best[0]):
            best = (len(d), name)
    return best[1] if best else None


def rust_files():
    for p in sorted(ROOT.rglob("*.rs")):
        if not SKIP & set(p.relative_to(ROOT).parts):
            yield p


_TOKENS = re.compile(
    r"""//[^\n]*|/\*.*?\*/|b?r(?P<h>\#*)".*?"(?P=h)|b?"(?:\\.|[^"\\])*"|b?'(?:\\.[^']*|[^\\'])'""",
    re.S,
)


def strip(src):
    """Source without comments and string/char literals (strings kept as empty ones)."""

    def blank(m):
        t = m.group(0)
        if t.startswith("//"):
            return ""
        if t.startswith("/*"):
            return "\n" * t.count("\n")
        return "''" if "'" == t.lstrip("b")[0] else '""'

    return _TOKENS.sub(blank, src)


def block_end(src, open_at):
    depth = 0
    for i in range(open_at, len(src)):
        if src[i] == "{":
            depth += 1
        elif src[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return len(src) - 1


def split_tests(src):
    """(non-test source, inline test source) of stripped `src`."""
    tests = []
    while (m := re.search(r"#\[cfg\(test\)\]\s*(pub\s+)?mod\s+\w+\s*\{", src)):
        end = block_end(src, m.end() - 1)
        tests.append(src[m.start():end + 1])
        src = src[:m.start()] + src[end + 1:]
    return src, "\n".join(tests)


def is_test_path(rel):
    return "/tests/" in "/" + rel or rel.endswith(("_tests.rs", "/tests.rs")) or "/benches/" in "/" + rel


def lines_cmd():
    dirs = crates()
    rows = defaultdict(lambda: [0, 0, 0])
    for p in rust_files():
        rel = p.relative_to(ROOT).as_posix()
        c = crate_of(rel, dirs)
        src, inline = split_tests(strip(p.read_text(errors="replace")))
        n = lambda s: sum(1 for l in s.splitlines() if l.strip())
        row = rows[c]
        if is_test_path(rel):
            row[1] += n(src) + n(inline)
        else:
            row[0] += n(src)
            row[1] += n(inline)
        row[2] += len(re.findall(r"#\[(?:tokio::)?test\]", src + inline))
    print(f"{'crate':28} {'source':>8} {'tests':>8} {'#[test]':>8}")
    for c, (s, t, k) in sorted(rows.items(), key=lambda kv: -kv[1][0]):
        print(f"{c or '?':28} {s:8} {t:8} {k:8}")
    print(f"{'total':28} {sum(r[0] for r in rows.values()):8} {sum(r[1] for r in rows.values()):8} {sum(r[2] for r in rows.values()):8}")


def functions():
    for p in rust_files():
        rel = p.relative_to(ROOT).as_posix()
        if is_test_path(rel):
            continue
        src, _ = split_tests(strip(p.read_text(errors="replace")))
        for m in re.finditer(r"\bfn\s+(\w+)\s*(?:<[^{;]*?>)?\s*\(", src):
            depth, i = 1, m.end()
            while i < len(src) and depth:
                depth += {"(": 1, ")": -1}.get(src[i], 0)
                i += 1
            params = src[m.end():i - 1]
            tail = src[i:i + 400]
            brace = re.search(r"[{;]", tail)
            if not brace or brace.group(0) == ";":
                continue
            start = i + brace.start()
            end = block_end(src, start)
            count = len([x for x in re.split(r",(?![^<(\[]*[>)\]])", params) if x.strip()])
            yield rel, m.group(1), src.count("\n", start, end) + 1, count


def fns_cmd():
    fs = list(functions())
    print("longest functions (lines)")
    for rel, name, n, _ in sorted(fs, key=lambda f: -f[2])[:25]:
        print(f"{n:6}  {rel}::{name}")
    print("\nmost parameters")
    for rel, name, _, k in sorted(fs, key=lambda f: -f[3])[:25]:
        print(f"{k:6}  {rel}::{name}")
    print(f"\n{len(fs)} functions; over 100 lines: {sum(1 for f in fs if f[2] > 100)}; over 7 parameters: {sum(1 for f in fs if f[3] > 7)}")


PATTERNS = {
    "unwrap()": r"\.unwrap\(\)",
    "expect(": r"\.expect\(",
    "clone()": r"\.clone\(\)",
    "unsafe": r"\bunsafe\b",
    "TODO": r"\bTODO\b",
    "#[allow(": r"#\[allow\(",
    "Box<dyn": r"Box<dyn\b",
    "Arc<Mutex": r"Arc<(?:Mutex|RwLock)",
    "static mut / OnceLock": r"static mut|OnceLock|OnceCell|LazyLock",
    "#[derive(Resource": r"#\[derive\([^)]*\bResource\b",
    "#[derive(Component": r"#\[derive\([^)]*\bComponent\b",
    "#[derive(Message": r"#\[derive\([^)]*\b(?:Message|Event)\b",
    "Query<(tuple": r"Query<\s*\(",
}


def patterns_cmd():
    dirs = crates()
    counts = defaultdict(Counter)
    widest = []
    for p in rust_files():
        rel = p.relative_to(ROOT).as_posix()
        if is_test_path(rel):
            continue
        src, _ = split_tests(strip(p.read_text(errors="replace")))
        c = crate_of(rel, dirs)
        for label, rx in PATTERNS.items():
            counts[label][c] += len(re.findall(rx, src))
        for m in re.finditer(r"Query<\s*\(", src):
            depth, i = 1, m.end()
            while i < len(src) and depth:
                depth += {"(": 1, ")": -1}.get(src[i], 0)
                i += 1
            widest.append((len(re.split(r",(?![^<(\[]*[>)\]])", src[m.end():i - 1])), rel))
    for label in PATTERNS:
        print(f"{sum(counts[label].values()):6}  {label}")
    print("\nwidest Query tuples")
    for n, rel in sorted(widest, reverse=True)[:10]:
        print(f"{n:6}  {rel}")


def structs_cmd():
    rows = []
    for p in rust_files():
        rel = p.relative_to(ROOT).as_posix()
        src, _ = split_tests(strip(p.read_text(errors="replace")))
        for m in re.finditer(r"#\[derive\([^)]*\b(Resource|Component|Message|Event)\b[^)]*\)\][^{;]*?\bstruct\s+(\w+)[^{;]*\{", src):
            body = src[m.end():block_end(src, m.end() - 1)]
            depth, fields, buf = 0, 0, ""
            for ch in body:
                depth += {"<": 1, "(": 1, "[": 1, ">": -1, ")": -1, "]": -1}.get(ch, 0)
                if ch == "," and depth == 0:
                    fields += 1 if ":" in buf else 0
                    buf = ""
                else:
                    buf += ch
            fields += 1 if ":" in buf else 0
            rows.append((fields, m.group(1), m.group(2), rel))
    for n, kind, name, rel in sorted(rows, reverse=True)[:25]:
        print(f"{n:4}  {kind:9} {name}  ({rel})")


def closures_cmd():
    md = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1"], cwd=ROOT, text=True))
    members = set(md["workspace_members"])
    names = {p["id"]: p["name"] for p in md["packages"] if p["id"] in members}
    deps = {n["id"]: [d for d in n["dependencies"] if d in members] for n in md["resolve"]["nodes"] if n["id"] in members}

    def closure(i, seen):
        for d in deps.get(i, []):
            if d not in seen:
                seen.add(d)
                closure(d, seen)
        return seen

    rows = sorted(((len(closure(i, set())), names[i]) for i in names), reverse=True)
    for n, name in rows:
        print(f"{n:4}  {name}")


def commits_cmd(limit):
    dirs = crates()
    log = subprocess.check_output(
        ["git", "log", "--no-merges", f"-{limit}", "--name-only", "--format=@@%h"], cwd=ROOT, text=True
    )
    # Lowest crate: the first of the layers, bottom up, that a commit touches.
    order = ["chimera_core", "chimera_world", "chimera_sim", "chimera_present", "chimera_net", "chimera_client"]
    tally, total = Counter(), 0
    for chunk in log.split("@@")[1:]:
        files = [f for f in chunk.splitlines()[1:] if f.endswith(".rs")]
        if not files:
            continue
        total += 1
        touched = {crate_of(f, dirs) for f in files}
        low = next((c for c in order if c in touched), None)
        tally[low or "other (leaf or tools)"] += 1
    for c, n in tally.most_common():
        print(f"{n:5}  {100 * n / total:5.1f}%  {c}")
    print(f"{total} commits touching Rust")


def radius_cmd(days):
    """How far a change reaches: the commits of the last `days` days by what
    they change, by the lowest layer they touch, and the number of workspace
    crates above the crates they touch (those whose tests a change can
    affect, by normal dependencies; `affected` adds dev-dependencies)."""
    md = metadata()
    names = {p["name"] for p in md["packages"]}
    dirs = crates()
    deps = {
        p["name"]: {d["name"] for d in p["dependencies"] if d["name"] in names and d.get("kind") is None} - {"workspace-hack"}
        for p in md["packages"]
    }

    def below(n, seen):
        for d in deps[n]:
            if d not in seen:
                seen.add(d)
                below(d, seen)
        return seen

    under = {n: below(n, set()) for n in names}
    above = {n: {m for m in names if n in under[m]} | {n} for n in names}

    def owner(f):
        return crate_of(f, dirs) or ("mashup" if f.startswith(("src/", "tests/")) else None)

    def kind(f):
        if f.startswith("specs/"):
            return "specs"
        if f.startswith("docs/") or f.endswith(".md"):
            return "docs"
        if f.startswith(("tools/", "scripts/", ".githooks/", ".claude/", ".config/", ".cargo/")):
            return "tooling"
        if f.endswith(".rs"):
            return "rust tests" if "/tests/" in f or f.startswith("tests/") else "rust source"
        if f.endswith("Cargo.toml") or f == "Cargo.lock":
            return "manifests"
        if f.endswith((".toml", ".ron", ".json", ".txt", ".csv")):
            return "data"
        return "other"

    def layer(crate):
        d = dirs[crate]
        top = d.split("/")[0]
        if top == "chimera":
            part = d.split("/")[1]
            if part in ("core", "world", "sim", "assets", "vfs", "formats"):
                return "Chimera's base (core, world, sim, assets, vfs, formats)"
            if part in ("client", "present", "ui"):
                return "Chimera's client side (client, present, ui)"
            return "chimera_net" if part == "net" else "Chimera, other (services, tools)"
        if top in ("source", "lithtech", "blam"):
            return "a loader"
        return "games only" if top == "games" else "apps only"

    order = [
        "Chimera's base (core, world, sim, assets, vfs, formats)",
        "chimera_net",
        "Chimera's client side (client, present, ui)",
        "Chimera, other (services, tools)",
        "a loader",
        "games only",
        "apps only",
    ]
    log = subprocess.check_output(
        ["git", "log", "--no-merges", f"--since={days} days ago", "--name-only", "--format=@@%h"], cwd=ROOT, text=True
    )
    files_by_kind, commits_by_kind, lowest, reach, direct = Counter(), Counter(), Counter(), [], []
    total = 0
    for chunk in log.split("@@")[1:]:
        files = [f for f in chunk.splitlines()[1:] if f]
        if not files:
            continue
        total += 1
        kinds = {kind(f) for f in files}
        for f in files:
            files_by_kind[kind(f)] += 1
        if "rust source" in kinds:
            commits_by_kind["touching Rust source"] += 1
        elif "rust tests" in kinds:
            commits_by_kind["Rust tests only"] += 1
        elif "data" in kinds:
            commits_by_kind["data only"] += 1
        else:
            commits_by_kind["docs, specs, tooling, other"] += 1
        touched = {owner(f) for f in files if kind(f) == "rust source"} - {None}
        if not touched:
            continue
        layers = {layer(c) for c in touched}
        lowest[next(name for name in order if name in layers)] += 1
        reached = set()
        for c in touched:
            reached |= above[c]
        reach.append(len(reached))
        direct.append(len(touched))
    print(f"{total} non-merge commits in {days} days")
    for k, n in commits_by_kind.most_common():
        print(f"{n:5}  {100 * n / total:5.1f}%  {k}")
    print("file changes: " + ", ".join(f"{n} {k}" for k, n in files_by_kind.most_common()))
    n = len(reach)
    if not n:
        return
    print(f"\nof the {n} commits touching Rust source, by the lowest layer touched:")
    for name in order:
        if lowest[name]:
            print(f"{lowest[name]:5}  {100 * lowest[name] / n:5.1f}%  {name}")
    reach.sort()
    direct.sort()
    print(
        f"\ncrates a commit can affect (of {len(names)}): median {reach[n // 2]}, mean {sum(reach) / n:.1f}; "
        f"crates it touches: median {direct[n // 2]}, mean {sum(direct) / n:.1f}"
    )
    for at in (10, 20, 30):
        print(f"{sum(1 for r in reach if r >= at):5}  {100 * sum(1 for r in reach if r >= at) / n:5.1f}%  can affect {at} crates or more")


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "lines":
        lines_cmd()
    elif cmd == "fns":
        fns_cmd()
    elif cmd == "patterns":
        patterns_cmd()
    elif cmd == "structs":
        structs_cmd()
    elif cmd == "closures":
        closures_cmd()
    elif cmd == "commits":
        commits_cmd(int(sys.argv[2]) if len(sys.argv) > 2 else 300)
    elif cmd == "radius":
        radius_cmd(int(sys.argv[2]) if len(sys.argv) > 2 else 14)
    else:
        print(__doc__)
        sys.exit(0 if cmd in ("", "-h", "--help") else 2)


if __name__ == "__main__":
    main()
