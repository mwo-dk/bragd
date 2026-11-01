# /// script
# requires-python = ">=3.10"
# dependencies = ["websocket-client"]
# ///
"""Pictures and GIFs of Bragd in the demo sandbox (sandbox.sh), on a display of its own: nothing
appears on the screen, and no key reaches another window (CLAUDE.md, Screenshots).

The app (`bragd-gui`, a release build) runs on GTK's Broadway display, shown by a headless
Chromium. Keys go to that page through the DevTools protocol (Input.dispatchKeyEvent), pictures
come from it (Page.captureScreenshot), clipped to the window. The demo home has default Settings,
so the Racing green Theme and Board theme show. A GIF is frames taken ~12 times a second, then
ffmpeg with palettegen/paletteuse.

    from cap import Desktop, gif, widths
    with Desktop() as d:                                  # or Desktop(size=(390, 844))
        d.keys("Ctrl+n"); d.shot("new-game.png")          # d.app(js) for what has no key
        frames = d.record(6, during=lambda: d.keys("e", "2", "e", "4"))
    gif(frames, "first-move.gif")                         # and first-move.png, its first frame
    widths("new-game", lambda d: d.keys("Ctrl+n"))        # new-game-390.png and new-game-1100.png

The window takes the display's size, which the app reads from BRAGD_WINDOW_SIZE (`WxH`). The
app runs with BRAGD_OFFLINE=1, so it checks for no update. A demo home without a state.json gets
one that has seen the install wizard, the tour cards, the hints and a Tip (SEEN), so they do not cover
the screen; `Desktop(state={...})` writes the state given instead, `{}` for a first start.
BRAGD_OFFLINE=1 also means no tablebase download or lookup is made.

The demo home is $DEMO_HOME (default <tmp>/bragd-cap-<uid>/demo-home, made with demo-home.sh).
`SANDBOX_ARGS` (extra bwrap arguments) is passed to sandbox.sh; keep the network, the page is
reached over 127.0.0.1. Run with `uv run --with websocket-client python my-script.py` from this
folder. Linux only; needs bwrap, broadwayd, chromium (or the browser $CHROMIUM names), cc, ffmpeg
and ImageMagick (magick).
"""

import base64, json, os, shutil, socket, subprocess, sys, tempfile, threading, time, urllib.request
from pathlib import Path

import websocket

HERE = Path(__file__).resolve().parent
SANDBOX = HERE / "sandbox.sh"
RUNTIME = Path(tempfile.gettempdir()) / f"bragd-cap-{os.getuid()}"
# The two widths every UI change is looked at (CLAUDE.md, Screenshots): a phone and a laptop.
WIDTHS = {390: 844, 1100: 760}
# Chromium talks only to this machine: no host name resolves but 127.0.0.1, and its own
# background calls (sync, updates, push messaging, metrics) are off.
OFFLINE = ["--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1", "--disable-background-networking",
           "--disable-component-update", "--disable-sync", "--disable-domain-reliability", "--disable-breakpad",
           "--no-pings", "--no-first-run", "--no-default-browser-check", "--metrics-recording-only"]


def _env(**extra):
    RUNTIME.mkdir(mode=0o700, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "LANG", "DEMO_HOME", "SANDBOX_ARGS")}
    # A runtime folder of its own: no Wayland, no session bus, no systemd of the user's reachable.
    env.update(XDG_RUNTIME_DIR=str(RUNTIME), WAYLAND_DISPLAY="", DISPLAY="", **extra)
    env.setdefault("DEMO_HOME", str(RUNTIME / "demo-home"))
    # Its own process namespace: when the sandbox stops, so does all it started (the Helper too).
    env["SANDBOX_ARGS"] = "--unshare-pid --die-with-parent " + env.get("SANDBOX_ARGS", "")
    return env


def _free_port():
    """A port nothing listens on now: two runs at once never share one."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _answers(port):
    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


def _stop(p):
    """Stop a process started here, by its own PID (CLAUDE.md: never by name or group). The
    sandbox's own process namespace takes what it started with it."""
    if p and p.poll() is None:
        p.terminate()
        try:
            p.wait(5)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait(5)


class StartupError(TimeoutError):
    """A part of the sandbox (Broadway and the app, or Chromium) did not start: says which, and
    where its log is. A TimeoutError, so Desktop starts again."""


# A start that fails for good is one plain line, not a stack trace.
_hook = sys.excepthook
sys.excepthook = lambda kind, e, tb: print(f"error: {e}", file=sys.stderr) if issubclass(kind, StartupError) else _hook(kind, e, tb)


def _until(what, probe, proc, log, deadline=60):
    """`probe()`'s first true answer, asked every 0.2 s for `deadline` seconds. A probe that
    raises has no answer yet. Fails at once when `proc`, which runs `what`, has stopped."""
    end, last = time.monotonic() + deadline, None
    while time.monotonic() < end:
        if proc.poll() is not None:
            hint = ""
            if "Socket path too long" in Path(log).read_text(errors="replace"):
                hint = " Its socket path is too long: set a short TMPDIR, such as ~/tmp/cap."
            raise StartupError(f"{what} stopped (exit {proc.returncode}) before it answered: see {log}.{hint}")
        try:
            if answer := probe():
                return answer
        except (OSError, ValueError, KeyError, websocket.WebSocketException) as e:
            last = e
        time.sleep(0.2)
    raise StartupError(f"{what} did not answer within {deadline} s{f' ({last})' if last else ''}: see {log}")


# Keys by name, as the DevTools protocol wants them: (key, code, windowsVirtualKeyCode).
NAMED = {"Enter": ("Enter", "Enter", 13), "Esc": ("Escape", "Escape", 27), "Tab": ("Tab", "Tab", 9),
         "Backspace": ("Backspace", "Backspace", 8), "Up": ("ArrowUp", "ArrowUp", 38), "Down": ("ArrowDown", "ArrowDown", 40),
         "Left": ("ArrowLeft", "ArrowLeft", 37), "Right": ("ArrowRight", "ArrowRight", 39), "Home": ("Home", "Home", 36),
         "End": ("End", "End", 35), "PageDown": ("PageDown", "PageDown", 34), "PageUp": ("PageUp", "PageUp", 33),
         "Insert": ("Insert", "Insert", 45), "Delete": ("Delete", "Delete", 46), "Space": (" ", "Space", 32)}
NAMED.update({f"F{i}": (f"F{i}", f"F{i}", 111 + i) for i in range(1, 13)})
MODS = {"Alt": 1, "Ctrl": 2, "Meta": 4, "Shift": 8}

# What a demo home has seen: the install wizard, every tour card, and the hints; and a Tip shown
# in 2100, so no Tip covers a picture.
SEEN = {"guide_seen": True, "seen_version": "999.0.0", "hints_shown": {h: 99 for h in ("sidebar", "features", "wizard")},
        "tip_shown": 4102444800}


# Broadway has no screen resolution (-1), which WebKitGTK turns into a page zoom of -1/96: the
# page lays out at a negative size. The app is started with this (and only the app: WebKit's own
# processes start without it).
DPI = r"""
#include <stdlib.h>
double gdk_screen_get_resolution(void *screen) { (void)screen; return 96.0; }
__attribute__((constructor)) static void only_here(void) { unsetenv("LD_PRELOAD"); }
"""


def _dpi():
    so = RUNTIME / "dpi96.so"
    if not so.exists():
        subprocess.run(["cc", "-shared", "-fPIC", "-O2", "-x", "c", "-", "-o", str(so)], input=DPI, text=True, check=True)
    return so


class Desktop:
    def __init__(self, args=(), lang="en_GB.UTF-8", size=(1440, 900), state=None):
        self.args, self.lang, self.size, self.state = list(args), lang, size, state
        self.procs, self.n, self.ws, self.wk, self.target, self.profile, self.logs = [], 0, None, None, None, None, []

    def __enter__(self):
        # A start that does not draw its window is started again, up to five times.
        for attempt in range(5):
            try:
                self._start()
                return self.ready()
            except TimeoutError as e:
                if attempt == 4:
                    self.__exit__(e)
                    raise
                self.__exit__()
                self.procs, self.ws, self.wk, self.target = [], None, None, None

    def _start(self):
        # Ports of its own for each start (Broadway, Chromium's DevTools, WebKit's inspector), and a
        # display named after its port: a second run at the same time never reaches this one's app.
        self.port, self.debug, self.inspector = _free_port(), _free_port(), _free_port()
        self.display = f":{self.port}"
        busy = [p for p in (self.port, self.debug, self.inspector) if _answers(p)]
        if busy:
            raise RuntimeError(f"something already answers on 127.0.0.1:{busy}: not starting, so no key or picture goes to its app")
        env = _env()
        _dpi()
        st = Path(env["DEMO_HOME"]) / ".local/share/bragd/state.json"
        if self.state is not None or not st.exists():
            st.parent.mkdir(parents=True, exist_ok=True)
            st.write_text(json.dumps(SEEN if self.state is None else self.state))
        w, h = self.size
        # broadwayd in the sandbox too: GTK hands it the pictures through /dev/shm, which is the
        # sandbox's own. The app waits for the page, so the display has its size first.
        go = Path(env["DEMO_HOME"]) / f".cap-go-{self.port}"
        go.unlink(missing_ok=True)
        # GIO_USE_PROXY_RESOLVER=dummy (#133): WEBKIT_INSPECTOR_HTTP_SERVER makes WebKit open a
        # socket inside its class init, which holds GObject's class lock. GIO's default proxy
        # resolver (libproxy) then waits for D-Bus, which dconf's thread is setting up and cannot
        # finish without that lock: about one start in four hung. Without the inspector, no hang.
        run = (f"broadwayd --address 127.0.0.1 --port {self.port} {self.display} >/dev/null 2>&1 & until [ -e /home/demo/{go.name} ]; do sleep 0.2; done; rm /home/demo/{go.name}; "
               f"exec env LD_PRELOAD={RUNTIME}/dpi96.so GDK_BACKEND=broadway BROADWAY_DISPLAY={self.display} WEBKIT_DISABLE_DMABUF_RENDERER=1 "
               f'GIO_USE_PROXY_RESOLVER=dummy WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:{self.inspector} BRAGD_WINDOW_SIZE={w}x{h} BRAGD_OFFLINE=1 LANG={self.lang} bragd-gui "$@"')
        gui_log, chromium_log = RUNTIME / f"gui-{self.port}.log", RUNTIME / f"chromium-{self.port}.log"
        self.logs = [open(gui_log, "w"), open(chromium_log, "w")]
        sandbox = subprocess.Popen([str(SANDBOX), "sh", "-c", run, "sh", *self.args], env=env, stdout=self.logs[0], stderr=subprocess.STDOUT)
        self.procs.append(sandbox)
        _until("Broadway (in the sandbox)", lambda: urllib.request.urlopen(f"http://127.0.0.1:{self.port}/", timeout=1), sandbox, gui_log)
        self.profile = tempfile.mkdtemp(prefix="cap-chromium-", dir=RUNTIME)
        chromium = subprocess.Popen([os.environ.get("CHROMIUM", "chromium"), "--headless=new", *OFFLINE, "--no-sandbox", "--hide-scrollbars", f"--user-data-dir={self.profile}",
                                     f"--window-size={w + 100},{h + 300}", f"--remote-debugging-port={self.debug}", f"http://127.0.0.1:{self.port}/"],
                                    # Its org.chromium.Chromium.* temp folders go in the profile, removed on exit.
                                    stdout=self.logs[1], stderr=subprocess.STDOUT, env={**os.environ, "TMPDIR": self.profile})
        self.procs.append(chromium)

        def devtools():
            # The page's target shows in /json some time after the endpoint itself answers.
            tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{self.debug}/json", timeout=2))
            url = next((t["webSocketDebuggerUrl"] for t in tabs if t["type"] == "page" and "webSocketDebuggerUrl" in t), None)
            return url and websocket.create_connection(url, suppress_origin=True)
        self.ws = _until("Chromium's DevTools endpoint", devtools, chromium, chromium_log)
        # Room for the window's title bar and its place on the display: a picture is clipped to
        # the page, and a window larger than the page loses its bottom and right edge: +160 cut
        # about 29 px off the bottom of a 1100 px picture, so the page is 300 px taller.
        self.call("Emulation.setDeviceMetricsOverride", width=w + 160, height=h + 300, deviceScaleFactor=1, mobile=False)
        time.sleep(1.5)
        go.touch()

    def __exit__(self, exc=None, *_):
        if self.wk:
            self.wk.close()
        for p in reversed(self.procs):
            _stop(p)
        if self.profile:
            shutil.rmtree(self.profile, ignore_errors=True)
        for log in self.logs:
            log.close()
            if exc is None:  # kept only when something went wrong
                Path(log.name).unlink(missing_ok=True)

    def call(self, method, **params):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method, "params": params}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                return r.get("result", r)

    def wait(self, secs):
        time.sleep(secs)

    def ready(self, timeout=60):
        """Wait until the window is drawn: a canvas of some size with something on it."""
        probe = "(()=>{const c=[...document.querySelectorAll('canvas')].find(c=>c.width>300);if(!c)return 0;const d=c.getContext('2d').getImageData(0,c.height/2,c.width,1).data;let n=0;for(let i=0;i<d.length;i+=4)if(d[i+3])n++;return n})()"
        end = time.time() + timeout
        while time.time() < end:
            if self.call("Runtime.evaluate", expression=probe, returnByValue=True)["result"].get("value", 0) > 300:
                time.sleep(2)
                # Focus: a click on the title bar, so keys reach the window.
                x, y, w, _ = self.frame()
                self.click(x + w * 0.3, y + 18)
                time.sleep(0.5)
                return self
            time.sleep(1)
        Path(RUNTIME / "not-drawn.png").write_bytes(self.png([0, 0, *self.size]))
        raise TimeoutError(f"the window was not drawn: see {RUNTIME}/not-drawn.png and gui-{self.port}.log")

    def app(self, expr, timeout=30):
        """Evaluate in the app's own page, through WebKit's inspector: for scrolling and clicking
        what has no key. A promise is waited for."""
        if not self.wk:
            self.wk = websocket.create_connection(f"ws://127.0.0.1:{self.inspector}/socket/1/1/WebPage", suppress_origin=True, timeout=5)
            try:
                while not self.target:
                    m = json.loads(self.wk.recv())
                    if m.get("method") == "Target.targetCreated" and m["params"]["targetInfo"].get("type") == "page":
                        self.target = m["params"]["targetInfo"]["targetId"]
            except websocket.WebSocketTimeoutException:
                pass
        self.n += 1
        n = self.n
        inner = {"id": n, "method": "Runtime.evaluate", "params": {"expression": expr, "returnByValue": True, "awaitPromise": True}}
        self.wk.send(json.dumps({"id": n, "method": "Target.sendMessageToTarget", "params": {"targetId": self.target, "message": json.dumps(inner)}}))
        end = time.time() + timeout
        while time.time() < end:
            try:
                m = json.loads(self.wk.recv())
            except websocket.WebSocketTimeoutException:
                continue
            if m.get("method") == "Target.dispatchMessageFromTarget":
                r = json.loads(m["params"]["message"])
                if r.get("id") == n:
                    res = r.get("result", {})
                    if res.get("wasThrown"):
                        raise RuntimeError(res["result"].get("description"))
                    return res.get("result", {}).get("value")
        raise TimeoutError(expr)

    def keys(self, *combos, pause=0.4):
        """Press keys: "Ctrl+n", "F1", "Enter", "Alt+Down"."""
        for combo in combos:
            *mods, k = combo.split("+") if combo != "+" else ["+"]
            m = sum(MODS[x] for x in mods)
            # Only letters and digits share their character's code as a virtual key: `.` is 46,
            # which is Delete, `-` Insert and `(` Down. Other characters go as text with none.
            key, code, vk = NAMED.get(k) or (k, f"Key{k.upper()}" if k.isalpha() else "", ord(k.upper()) if k.isascii() and k.isalnum() else 0)
            text = {"Enter": "\r"}.get(key, key if len(key) == 1 and not (m & 3) else "")
            for x in mods:
                self.call("Input.dispatchKeyEvent", type="rawKeyDown", key=x if x != "Ctrl" else "Control", modifiers=m)
            self.call("Input.dispatchKeyEvent", type="keyDown" if text else "rawKeyDown", key=key, code=code, windowsVirtualKeyCode=vk, modifiers=m, text=text)
            self.call("Input.dispatchKeyEvent", type="keyUp", key=key, code=code, windowsVirtualKeyCode=vk, modifiers=m)
            for x in reversed(mods):
                self.call("Input.dispatchKeyEvent", type="keyUp", key=x if x != "Ctrl" else "Control", modifiers=0)
            time.sleep(pause)

    def type(self, text, gap=0.08):
        for ch in text:
            self.keys("Space" if ch == " " else ch, pause=gap)

    def click(self, x, y):
        for t in ("mousePressed", "mouseReleased"):
            self.call("Input.dispatchMouseEvent", type=t, x=x, y=y, button="left", clickCount=1)

    def frame(self):
        """The window's rectangle on the page: Broadway draws it at a place of its own."""
        r = self.call("Runtime.evaluate", expression="JSON.stringify([...document.querySelectorAll('canvas,div,img')].map(e => e.getBoundingClientRect()).filter(r => r.width > 300 && r.height > 300).map(r => [r.x, r.y, r.width, r.height])[0] || null)", returnByValue=True)
        v = json.loads(r["result"]["value"])
        return v or [0, 0, *self.size]

    def png(self, clip=None):
        x, y, w, h = clip or self.frame()
        r = self.call("Page.captureScreenshot", format="png", clip={"x": x, "y": y, "width": w, "height": h, "scale": 1})
        return base64.b64decode(r["data"])

    def shot(self, out, clip=None):
        Path(out).write_bytes(self.png(clip))

    def record(self, secs, during=None, fps=12, width=800):
        """Frames for `secs` seconds while `during` runs (in a thread), `width` wide; [(time, png)]."""
        clip, frames, t0 = self.frame(), [], time.time()
        th = threading.Thread(target=during) if during else None
        # Its own connection: keys go on the main one meanwhile.
        tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{self.debug}/json"))
        ws = websocket.create_connection([t for t in tabs if t["type"] == "page"][0]["webSocketDebuggerUrl"], suppress_origin=True)
        if th:
            th.start()
        i = 10_000
        while time.time() - t0 < secs:
            i += 1
            ws.send(json.dumps({"id": i, "method": "Page.captureScreenshot", "params": {"format": "png", "clip": {"x": clip[0], "y": clip[1], "width": clip[2], "height": clip[3], "scale": width / clip[2]}}}))
            while True:
                r = json.loads(ws.recv())
                if r.get("id") == i:
                    frames.append((time.time() - t0, base64.b64decode(r["result"]["data"])))
                    break
            time.sleep(max(0, t0 + len(frames) / fps - time.time()))
        if th:
            th.join()
        ws.close()
        return frames


def widths(name, setup=lambda d: None, args=(), out=HERE, state=None):
    """`<name>-390.png` and `<name>-1100.png`: the app started at each width, `setup(d)` run to
    reach the screen, then captured. Look at both before calling a UI change done."""
    made = []
    for w, h in WIDTHS.items():
        with Desktop(args=args, size=(w, h), state=state) as d:
            setup(d)
            d.wait(1)
            path = Path(out) / f"{name}-{w}.png"
            d.shot(path)
            made.append(path)
    return made


def gif(frames, out, width=800, fps=12, hold=0.6):
    """[(time, png)] as a looping GIF, `width` wide, `fps` a second; the first frame is held
    `hold` seconds at both ends (a calm start and end) and saved beside it as <name>.png.
    CLAUDE.md's limits: at most 8 s, 800 px wide, 10–12 fps, under 1.5 MB."""
    out = Path(out)
    tmp = Path(tempfile.mkdtemp(prefix="cap-gif-", dir=RUNTIME))
    first = frames[0][1]
    subprocess.run(["magick", "png:-", "-resize", f"{width}x", str(out.with_suffix(".png"))], input=first, check=True)
    # Even steps: at each tick the latest frame taken by then.
    end = frames[-1][0]
    ticks, j = [], 0
    for k in range(int(end * fps) + 1):
        while j + 1 < len(frames) and frames[j + 1][0] <= k / fps:
            j += 1
        ticks.append(frames[j][1])
    seq = [first] * int(hold * fps) + ticks + [first] * int(hold * fps)
    for i, f in enumerate(seq):
        (tmp / f"{i:04}.png").write_bytes(f)
    vf = f"scale={width}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-framerate", str(fps), "-i", str(tmp / "%04d.png"), "-vf", vf, "-loop", "0", str(out)], check=True)
    shutil.rmtree(tmp, ignore_errors=True)
    size = out.stat().st_size
    if size > 1_500_000:
        print(f"warning: {out.name} is {size} bytes, over CLAUDE.md's 1.5 MB")
    return size


def self_test():
    """_until waits through a probe that fails, then answers; and fails, naming what and its
    log, when the process stops or the deadline passes."""
    log = Path(tempfile.mkstemp(prefix="cap-self-test-")[1])
    try:
        log.write_text("[1:2:ERROR:process_singleton_posix.cc] Socket path too long: /x\n")
        tries = iter([OSError("refused"), ValueError("half a reply"), None, "ws"])

        def probe():
            t = next(tries)
            if isinstance(t, Exception):
                raise t
            return t
        alive = subprocess.Popen(["sleep", "30"])
        try:
            assert _until("Thing", probe, alive, log, deadline=5) == "ws"
            try:
                _until("Thing", lambda: None, alive, log, deadline=0.5)
                raise AssertionError("no StartupError at the deadline")
            except StartupError as e:
                assert "Thing did not answer within 0.5 s" in str(e) and str(log) in str(e), e
        finally:
            _stop(alive)
        dead = subprocess.Popen(["false"])
        dead.wait()
        try:
            _until("Chromium", lambda: "never asked", dead, log)
            raise AssertionError("no StartupError for a stopped process")
        except StartupError as e:
            assert "Chromium stopped (exit 1)" in str(e) and "short TMPDIR" in str(e) and str(log) in str(e), e
    finally:
        log.unlink()
    print("cap self-test: ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        sys.exit()
    # The look check after a UI change: the first screen at both widths.
    for p in widths("start"):
        print(p)
