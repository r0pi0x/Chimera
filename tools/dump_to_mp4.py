#!/usr/bin/env python3
"""A client's frame dump as an MP4 with its sound (docs/OBSERVABILITY.md,
section 2, "Offscreen").

An offscreen run with `--dump <dir> --dump-every 1` leaves in <dir> the
frames (frame_NNNNNN.png), the sound (audio.wav, mixed a frame at a time)
and frames.tsv: each frame's sample position in audio.wav, where that
frame's sounds start. This script shows each frame for exactly the time
until the next one's position, so picture and sound line up however
slowly the frames rendered, and encodes H.264 + AAC with ffmpeg:

    tools/dump_to_mp4.py <dir> [-o <dir>/clip_audio.mp4] [--fps 30] [--crf 20]
                         [--from <frame>] [--to <frame>]

--from/--to keep only those frames (numbers as in the file names, --to
excluded), e.g. to leave the loading screen out; the sound follows.

--fps is the output's constant rate (default: the frames' median rate,
rounded). Works on Linux and Windows (Python 3, ffmpeg on the PATH).
"""
import argparse
import os
import statistics
import subprocess
import sys
import tempfile


def read_positions(path):
    rate = None
    rows = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.startswith("#"):
                words = line.replace(",", " ").split()
                if "second" in words:
                    rate = int(words[words.index("second") - 2])
                continue
            if line:
                name, pos = line.split("\t")
                rows.append((name, int(pos)))
    if rate is None:
        sys.exit(f"{path}: no rate in the header")
    return rate, rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("-o", "--out")
    ap.add_argument("--fps", type=float)
    ap.add_argument("--crf", type=int, default=20)
    ap.add_argument("--from", dest="first", type=int, default=0)
    ap.add_argument("--to", dest="end", type=int)
    args = ap.parse_args()
    d = os.path.abspath(args.dir)
    out = os.path.abspath(args.out or os.path.join(d, "clip_audio.mp4"))
    tsv, wav = os.path.join(d, "frames.tsv"), os.path.join(d, "audio.wav")
    for p in (tsv, wav):
        if not os.path.exists(p):
            sys.exit(f"{p} missing: record with --offscreen <WxH> --dump <dir> --dump-every 1")
    rate, rows = read_positions(tsv)
    # The last frame or two may not have been written when the run ended.
    rows = [r for r in rows if os.path.exists(os.path.join(d, r[0]))]
    number = lambda r: int(r[0].rsplit("_", 1)[1].split(".")[0])
    rows = [r for r in rows if number(r) >= args.first and (args.end is None or number(r) < args.end)]
    if len(rows) < 2:
        sys.exit("fewer than two frames")
    steps = [b[1] - a[1] for a, b in zip(rows, rows[1:])]
    step = statistics.median(steps) or 1
    fps = args.fps or max(1, round(rate / step))
    # ffmpeg's concat list: each frame for its time; the last one for the
    # median frame's, and named twice (the demuxer ignores the last duration).
    lines = ["ffconcat version 1.0"]
    for (name, pos), nxt in zip(rows, [r[1] for r in rows[1:]] + [rows[-1][1] + step]):
        lines.append(f"file '{os.path.join(d, name).replace(os.sep, '/')}'")
        lines.append(f"duration {(nxt - pos) / rate:.6f}")
    lines.append(f"file '{os.path.join(d, rows[-1][0]).replace(os.sep, '/')}'")
    start = rows[0][1] / rate
    length = (rows[-1][1] + step - rows[0][1]) / rate
    with tempfile.NamedTemporaryFile("w", suffix=".ffconcat", delete=False, encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
        listing = f.name
    try:
        cmd = [
            "ffmpeg", "-y", "-loglevel", "error",
            "-f", "concat", "-safe", "0", "-i", listing,
            # The sound from the first frame's position on.
            "-ss", f"{start:.6f}", "-i", wav,
            "-map", "0:v", "-map", "1:a",
            "-fps_mode", "cfr", "-r", f"{fps:g}",
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", str(args.crf),
            "-c:a", "aac", "-b:a", "192k",
            "-t", f"{length:.6f}", "-movflags", "+faststart", out,
        ]
        subprocess.run(cmd, check=True)
    finally:
        os.unlink(listing)
    print(f"{out}: {len(rows)} frames, {length:.2f} s at {fps:g} fps, sound from {start:.3f} s of audio.wav")


if __name__ == "__main__":
    main()
