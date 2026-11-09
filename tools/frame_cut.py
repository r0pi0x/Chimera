#!/usr/bin/env python3
"""The worst single frames of a run and what is in them (docs/performance.md,
"Nothing blocks the main thread").

Run a game with CHIMERA_FRAME_LOG=<csv> (keep its log off a busy disk) under

    perf record -F 1999 -e cpu-clock:u -k CLOCK_MONOTONIC --call-graph fp -o perf.data -- <game> ...
    perf script --no-inline -F comm,tid,time,ip,sym -i perf.data > perf.txt

(a `profiling` build with frame pointers: tools/perf-profile.sh), then

    tools/frame_cut.py frames.csv [--perf perf.txt] [--top 10] [--skip 5] [--syms 25]

Ranks each world's frames by instructions, the main world's less its ticks' (a
frame after a stall runs many ticks; those show under `ticks`), and with --perf
lists the functions on that world's thread during each (callees included). The
main thread's samples inside the fixed ticks are left out of its lists.
"""
import argparse
import bisect
import collections
import csv
import re

SKIP = re.compile(
    r"^(core::|std::|alloc::|\[unknown\]|<alloc|<core|<std|_start|__libc|main$|clone|start_thread|bevy_ecs::schedule|"
    r"bevy_app::|<bevy_app|<bevy_ecs::schedule|<bevy_ecs::world::World>::(try_|resource_scope|schedule_scope)|bevy_tasks|"
    r"<bevy_tasks|async_|bevy_ecs::system::|<bevy_ecs::system|<bevy_winit|bevy_winit|<<bevy_winit|<winit|winit|calloop|"
    r"chimera_client::client::app::run|.*::main$|bevy_time::fixed|<async_task|<futures_lite|futures_lite)"
)


def frames(path):
    out = {"main": [], "render": []}
    with open(path) as f:
        for r in csv.DictReader(f):
            if r["world"] in out:
                ticks = int(r["tick_instructions"] or 0)
                out[r["world"]].append(
                    (float(r["begun_s"]), float(r["ms"]), float(r["cpu_ms"]), int(r["instructions"]) - ticks, int(r["ticks"]), ticks)
                )
    return out


def samples(path):
    head = re.compile(r"^(.+?)\s+(\d+)\s+(\d+\.\d+):")
    out, cur = [], None
    with open(path, errors="replace") as f:
        for line in f:
            if not line.strip():
                if cur:
                    out.append(cur)
                cur = None
            elif line[0] not in " \t":
                m = head.match(line)
                cur = (float(m[3]), m[1].strip(), int(m[2]), []) if m else None
            elif cur is not None:
                p = line.split(None, 1)
                if len(p) > 1:
                    cur[3].append(re.sub(r"\+0x[0-9a-f]+$", "", p[1].strip()))
    out.sort(key=lambda s: s[0])
    return out


def main():
    a = argparse.ArgumentParser()
    a.add_argument("frames")
    a.add_argument("--perf")
    a.add_argument("--top", type=int, default=10)
    a.add_argument("--skip", type=float, default=0.0, help="seconds to leave out at the start")
    a.add_argument("--syms", type=int, default=25)
    o = a.parse_args()
    rows = frames(o.frames)
    t0 = min((r[0][0] for r in rows.values() if r), default=0.0)
    all_samples = samples(o.perf) if o.perf else []
    times = [s[0] for s in all_samples]
    by_tid = collections.Counter((s[1], s[2]) for s in all_samples)
    main_tid = next((t for (c, t), _ in by_tid.most_common() if "Render" not in c and "Task" not in c and "Async" not in c), None)
    render_tids = {t for (c, t) in by_tid if c.startswith("Render")}
    for world, rs in rows.items():
        rs = [r for r in rs if r[0] - t0 >= o.skip]
        print(f"{world}: {len(rs)} frames")
        for r in sorted(rs, key=lambda r: -r[3])[: o.top]:
            print(f"  t+{r[0]-t0:8.2f} s {r[1]:8.1f} ms cpu {r[2]:7.1f} ms {r[3]/1e6:8.1f} M, {r[4]} ticks {r[5]/1e6:.0f} M")
            if not all_samples:
                continue
            i, j = bisect.bisect_left(times, r[0]), bisect.bisect_right(times, r[0] + r[1] / 1e3)
            inc = collections.Counter()
            for s in all_samples[i:j]:
                if not (s[2] == main_tid if world == "main" else s[2] in render_tids):
                    continue
                if world == "main" and any("run_fixed_main_schedule" in q for q in s[3]):
                    continue
                inc.update({q[:110] for q in s[3] if not SKIP.match(q)})
            for sym, n in inc.most_common(o.syms):
                print(f"      {n:5d} {sym}")


if __name__ == "__main__":
    main()
