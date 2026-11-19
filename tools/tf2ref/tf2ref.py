#!/usr/bin/env python3
"""Ours (Chimera project). Real TF2 as a reference for screenshots, with no
display (docs/OBSERVABILITY.md section 6, "TF2 as the reference").

The TF2 client runs as a listen server on 2fort (or any map) with SDL's
"offscreen" video driver and a Vulkan layer of ours (`headless_vk.c`) that
gives NVIDIA's driver headless surfaces, so it draws with no window system;
its own `jpeg` command saves the frames. TF2 needs the Steam client running
(logged in once on the machine); when no X display answers, Steam is started
on a rootless Xwayland of ours over `nullcomp.c`, a Wayland compositor that
shows nothing. TF2 is driven over RCON on 127.0.0.1:27115 (never 27015).

    tools/tf2ref/tf2ref.py up [--map ctf_2fort]   # Steam (if needed) + TF2, ready to drive
    tools/tf2ref/tf2ref.py cmd "setpos 0 0 0" "getpos"
    tools/tf2ref/tf2ref.py shot <name> --out <dir>
    tools/tf2ref/tf2ref.py scene tools/tf2ref/scenes/<x>.tf2 --out <dir>
    tools/tf2ref/tf2ref.py pair --ref <dir> --ours <dir> --out <dir>   # ref | ours | diff
    tools/tf2ref/tf2ref.py onset --dir <dir> --prefix smg_f --offsets 0,2 --names smg_2ms,smg_10ms
    tools/tf2ref/tf2ref.py down [--all]           # quit TF2 (and the Steam/X stack we started)

Scenes (`*.tf2`): a console line per line, `#` comments, and directives:
`sleep <wall s>`, `gsleep <game s>` (wall time = game time / the last
`host_timescale` the scene set), `shot <name>` (`<out>/<name>.png`),
`snap <name>` (a picture taken inside the batch, frame-exact),
`player <team> <class>`. Consecutive console lines (and snaps) go in one
RCON packet, a line each, so `wait` counts frames between them: with
`host_framerate 0.002` every frame is 2 ms of game time, which is how
effects are stepped (`host_timescale` is not: TF2's particles came out
wrong under it, flashes gone in 15 ms instead of 80). The frame is 1024x768 (SDL's offscreen display), so compare
with our `--offscreen 1024x768`; horizontal FOV equals `fov_desired` at 4:3.
Game imagery stays in scratch folders, never in the repository.
"""

import os
import shutil
import socket
import struct
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "chimera" / "tf2ref"
TF2 = Path(os.environ.get("CHIMERA_TF2_CLIENT", Path.home() / "games" / "tf2"))
STEAM = Path.home() / ".local/share/Steam"
SNIPER = STEAM / "steamapps/common/SteamLinuxRuntime_sniper/run"
PORT = int(os.environ.get("CHIMERA_TF2REF_PORT", "27115"))
PASSWORD = "chimera-tf2ref"
XDISPLAY = os.environ.get("CHIMERA_TF2REF_X", ":9")
WAYLAND = "chimera-null-0"
RUNTIME = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")


class Rcon:
    def __init__(self, timeout=10.0):
        self.s = socket.create_connection(("127.0.0.1", PORT), timeout=timeout)
        self.next_id = 1
        self._send(3, PASSWORD)
        while True:
            rid, kind, _ = self._recv()
            if kind == 2:
                if rid == -1:
                    raise RuntimeError("rcon auth failed")
                break

    def _send(self, kind, body):
        rid = self.next_id
        self.next_id += 1
        data = struct.pack("<ii", rid, kind) + body.encode() + b"\0\0"
        self.s.sendall(struct.pack("<i", len(data)) + data)
        return rid

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.s.recv(n - len(buf))
            if not chunk:
                raise RuntimeError("rcon connection closed")
            buf += chunk
        return buf

    def _recv(self):
        (size,) = struct.unpack("<i", self._read(4))
        data = self._read(size)
        rid, kind = struct.unpack("<ii", data[:8])
        return rid, kind, data[8:-2].decode(errors="replace")

    def exec(self, cmd):
        rid = self._send(2, cmd)
        marker = self._send(0, "")
        out = []
        while True:
            got, _, body = self._recv()
            if got == marker:
                break
            if got == rid:
                out.append(body)
        self._recv()
        return "".join(out)


def rcon(retries=1):
    for i in range(retries):
        try:
            return Rcon()
        except (OSError, RuntimeError):
            if i + 1 == retries:
                raise
            time.sleep(2)


def build():
    """Compiles the layer and the compositor into the cache (when changed)."""
    CACHE.mkdir(parents=True, exist_ok=True)
    so = CACHE / "libheadless_vk.so"
    src = HERE / "headless_vk.c"
    if not so.exists() or so.stat().st_mtime < src.stat().st_mtime:
        subprocess.run(["gcc", "-O2", "-shared", "-fPIC", "-U_FORTIFY_SOURCE", "-fvisibility=hidden",
                        "-o", str(so), str(src), "-lpthread"], check=True)
    (CACHE / "headless_vk.json").write_text(f"""{{
  "file_format_version": "1.1.2",
  "layer": {{
    "name": "VK_LAYER_CHIMERA_headless",
    "type": "GLOBAL",
    "library_path": "{so}",
    "api_version": "1.3.0",
    "implementation_version": "1",
    "description": "Headless surfaces on drivers without VK_EXT_headless_surface",
    "instance_extensions": [{{"name": "VK_EXT_headless_surface", "spec_version": "1"}}],
    "enable_environment": {{"CHIMERA_HEADLESS_VK": "1"}},
    "disable_environment": {{"CHIMERA_HEADLESS_VK_OFF": "1"}}
  }}
}}
""")
    comp = CACHE / "nullcomp"
    src = HERE / "nullcomp.c"
    if not comp.exists() or comp.stat().st_mtime < src.stat().st_mtime:
        flags = subprocess.run(["pkg-config", "--cflags", "--libs", "wayland-server"],
                               capture_output=True, text=True, check=True).stdout.split()
        subprocess.run(["gcc", "-O2", "-o", str(comp), str(src), *flags], check=True)


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def pidfile(name):
    return CACHE / f"{name}.pid"


def started(name):
    p = pidfile(name)
    if p.exists():
        try:
            pid = int(p.read_text())
            if alive(pid):
                return pid
        except ValueError:
            pass
    return None


def spawn(name, args, env, log):
    with open(CACHE / f"{name}.log", "w") as out:
        proc = subprocess.Popen(args, env=env, stdout=out, stderr=subprocess.STDOUT,
                                stdin=subprocess.DEVNULL, start_new_session=True)
    pidfile(name).write_text(str(proc.pid))
    if log:
        print(f"tf2ref: started {name} (pid {proc.pid})")
    return proc.pid


def steam_running():
    try:
        pid = int((Path.home() / ".steam/steam.pid").read_text())
    except (OSError, ValueError):
        return False
    return alive(pid) and Path(f"/proc/{pid}/comm").read_text().strip().startswith("steam")


def x_works(display):
    env = dict(os.environ, DISPLAY=display)
    return subprocess.run(["xprop", "-root", "_XKB_RULES_NAMES"], env=env,
                          capture_output=True).returncode == 0


def up_steam():
    if steam_running():
        return
    display = os.environ.get("DISPLAY")
    if not (display and x_works(display)):
        display = XDISPLAY
        if not x_works(display):
            env = dict(os.environ, XDG_RUNTIME_DIR=RUNTIME)
            if not started("nullcomp"):
                spawn("nullcomp", [str(CACHE / "nullcomp"), WAYLAND], env, True)
                time.sleep(1)
            env["WAYLAND_DISPLAY"] = WAYLAND
            spawn("xwayland", ["Xwayland", display, "-rootless", "-noreset", "-nolisten", "tcp", "-shm"],
                  env, True)
            for _ in range(30):
                if x_works(display):
                    break
                time.sleep(0.5)
            else:
                sys.exit("tf2ref: our Xwayland did not come up (see the cache's xwayland.log)")
    env = {k: v for k, v in os.environ.items() if k != "WAYLAND_DISPLAY"}
    env.update(DISPLAY=display, XDG_RUNTIME_DIR=RUNTIME)
    spawn("steam", ["steam", "-silent"], env, True)
    for _ in range(120):
        if steam_running() and subprocess.run(["pgrep", "-x", "steamwebhelper"],
                                              capture_output=True).returncode == 0:
            break
        time.sleep(1)
    else:
        sys.exit("tf2ref: Steam did not start")
    time.sleep(15)  # its login and the IPC pipe


def game_pid():
    out = subprocess.run(["pgrep", "-x", "tf_linux64"], capture_output=True, text=True).stdout.split()
    return int(out[0]) if out else None


def status_ok(r, map_name):
    s = r.exec("status")
    return f"map     : {map_name}" in s


def up(map_name, extra):
    build()
    if game_pid():
        print("tf2ref: TF2 already running; reusing it")
    else:
        up_steam()
        env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY")}
        env.update(SteamAppId="440", SteamGameId="440", SDL_VIDEODRIVER="offscreen",
                   CHIMERA_HEADLESS_VK="1", VK_ADD_IMPLICIT_LAYER_PATH=str(CACHE),
                   VK_IMPLICIT_LAYER_PATH=str(CACHE), XDG_RUNTIME_DIR=RUNTIME)
        args = [str(SNIPER), "--", "./tf.sh", "-game", "tf", "-novid", "-windowed", "-insecure",
                "-nobreakpad", "-condebug", "-usercon", "-port", str(PORT), "+ip", "127.0.0.1",
                "+rcon_password", PASSWORD, "+volume", "0", "+maxplayers", "8", "+sv_cheats", "1",
                "+map", map_name, *extra]
        with open(CACHE / "tf2.log", "w") as out:
            proc = subprocess.Popen(args, cwd=TF2, env=env, stdout=out, stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL, start_new_session=True)
        pidfile("tf2").write_text(str(proc.pid))
        print(f"tf2ref: starting TF2 on {map_name}")
    deadline = time.time() + 300
    while time.time() < deadline:
        if not game_pid() and time.time() > deadline - 280:
            sys.exit("tf2ref: TF2 exited (see the cache's tf2.log)")
        try:
            r = Rcon(timeout=20)
            if status_ok(r, map_name):
                break
        except (OSError, RuntimeError):
            pass
        time.sleep(3)
    else:
        sys.exit("tf2ref: timed out waiting for the map")
    print(f"tf2ref: TF2 ready on {map_name} (rcon 127.0.0.1:{PORT})")


def setup_player(r, team="red", cls="soldier"):
    for line in ["hidepanel info", f"jointeam {team}", f"joinclass {cls}"]:
        r.exec(line)
        time.sleep(1)
    for line in ["hidepanel class_red", "hidepanel class_blue", "hidepanel team", "sv_cheats 1",
                 "mp_waitingforplayers_cancel 1", "mp_autoteambalance 0", "mp_teams_unbalance_limit 0",
                 "jpeg_quality 100", "cl_showfps 0", "net_graph 0", "tf_bot_quota 0"]:
        r.exec(line)


def collect(name, out, wait=10.0):
    """TF2's `jpeg <name>` file into `<out>/<name>.png`."""
    src = TF2 / "tf/screenshots" / f"{name}.jpg"
    deadline = time.time() + wait
    while time.time() < deadline:
        if src.exists() and src.stat().st_size > 0:
            break
        time.sleep(0.05)
    else:
        print(f"  missing {name}")
        return
    time.sleep(0.1)
    dest = out / f"{name}.png"
    subprocess.run(["magick", str(src), str(dest)], check=True)
    src.unlink()
    print(f"  shot {dest}")


def shot(r, name, out):
    out.mkdir(parents=True, exist_ok=True)
    (TF2 / "tf/screenshots" / f"{name}.jpg").unlink(missing_ok=True)
    r.exec(f"jpeg {name} 100")
    collect(name, out)


def scene(path, out):
    r = rcon(3)
    out.mkdir(parents=True, exist_ok=True)
    timescale = 1.0
    batch = []
    snaps = []

    def flush():
        if batch:
            # As a chain of aliases, each its own short RCON line: RCON
            # cuts a long `;` chain and drops the connection on a long
            # packet, and an exec'd cfg ran its `wait`s in one frame. Each
            # alias runs its line, then the next alias (after the line's
            # `wait`, if it is one).
            for i, line in enumerate(batch):
                tail = f";ctr_{i + 1}" if i + 1 < len(batch) else ""
                r.exec(f'alias ctr_{i} "{line}{tail}"')
            reply = r.exec("ctr_0")
            with open(out / "scene.log", "a") as log:
                log.write(" | ".join(batch) + "\n")
                for line in reply.splitlines():
                    if line.strip():
                        log.write(f"| {line}\n")
            batch.clear()
        for name in snaps:
            collect(name, out, 30.0)
        snaps.clear()

    (out / "scene.log").write_text("")
    for raw in Path(path).read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        words = line.split()
        if words[0] == "sleep":
            flush()
            time.sleep(float(words[1]))
        elif words[0] == "gsleep":
            flush()
            time.sleep(float(words[1]) / max(timescale, 1e-4))
        elif words[0] == "shot":
            flush()
            shot(r, words[1], out)
        elif words[0] == "snap":
            # A picture inside the batch (frame-exact with `wait` and
            # `host_framerate`), collected after it.
            (TF2 / "tf/screenshots" / f"{words[1]}.jpg").unlink(missing_ok=True)
            batch.append(f"jpeg {words[1]} 100")
            snaps.append(words[1])
        elif words[0] == "player":
            flush()
            setup_player(r, *words[1:3])
        else:
            if words[0] == "host_timescale" and len(words) > 1:
                timescale = float(words[1])
            batch.append(line)
    flush()


def pair(ref, ours, out):
    """ref | ours | |difference| x4 for every name found in both folders."""
    out.mkdir(parents=True, exist_ok=True)
    for a in sorted(Path(ref).glob("*.png")):
        b = Path(ours) / a.name
        if not b.exists():
            continue
        w, h = subprocess.run(["magick", "identify", "-format", "%w %h", str(a)],
                              capture_output=True, text=True, check=True).stdout.split()
        dest = out / f"pair_{a.stem}.png"
        subprocess.run(["magick", str(a), "(", str(b), "-resize", f"{w}x{h}!", ")",
                        "(", "-clone", "0", "-clone", "1", "-compose", "difference", "-composite",
                        "-evaluate", "multiply", "4", ")", "+append", str(dest)], check=True)
        mae = subprocess.run(["magick", "compare", "-metric", "MAE", str(a), "(", str(b), "-resize",
                              f"{w}x{h}!", ")", "null:"], capture_output=True, text=True).stderr
        print(f"  {a.stem}: MAE {mae.strip()} -> {dest}")


def onset(folder, prefix, offsets, names):
    """The first of `<prefix>NN.png` that differs from the first one (the
    shot's frame: a burst of snaps after `+attack`), then copies of the
    frames `offsets` after it as `names` (`.png`)."""
    frames = sorted(Path(folder).glob(f"{prefix}[0-9][0-9].png"))
    if not frames:
        sys.exit(f"tf2ref: no {prefix}NN.png in {folder}")
    first = None
    for i, f in enumerate(frames[1:], 1):
        ae = subprocess.run(["magick", "compare", "-metric", "AE", "-fuzz", "12%", str(frames[0]), str(f), "null:"],
                            capture_output=True, text=True).stderr.split()[0]
        if float(ae) > 200:
            first = i
            break
    if first is None:
        sys.exit(f"tf2ref: {prefix}: no frame changed")
    print(f"  {prefix}: the shot shows at frame {first}")
    for off, name in zip(offsets, names):
        i = min(first + off, len(frames) - 1)
        shutil.copy(frames[i], Path(folder) / f"{name}.png")


def down(everything):
    try:
        Rcon(timeout=5).exec("quit")
        time.sleep(3)
    except (OSError, RuntimeError):
        pass
    pid = game_pid()
    if pid:
        os.kill(pid, 15)
    if everything:
        if started("steam") and steam_running():
            subprocess.run(["steam", "-shutdown"], capture_output=True)
            time.sleep(5)
        for name in ["xwayland", "nullcomp"]:
            pid = started(name)
            if pid:
                os.kill(pid, 15)
    print("tf2ref: down")


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)

    def opt(name, default=None):
        if name in args:
            i = args.index(name)
            v = args[i + 1]
            del args[i:i + 2]
            return v
        return default

    cmd = args.pop(0)
    if cmd == "up":
        m = opt("--map", "ctf_2fort")
        up(m, args)
        setup_player(rcon(3), opt("--team", "red"), opt("--class", "soldier"))
    elif cmd == "cmd":
        r = rcon(3)
        for line in args:
            print(r.exec(line))
    elif cmd == "shot":
        out = Path(opt("--out", "."))
        shot(rcon(3), args[0], out)
    elif cmd == "scene":
        out = Path(opt("--out", "."))
        scene(args[0], out)
    elif cmd == "pair":
        pair(opt("--ref"), opt("--ours"), Path(opt("--out", ".")))
    elif cmd == "onset":
        folder, prefix = opt("--dir"), opt("--prefix")
        offsets = [int(x) for x in opt("--offsets").split(",")]
        onset(folder, prefix, offsets, opt("--names").split(","))
    elif cmd == "down":
        down("--all" in args)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
