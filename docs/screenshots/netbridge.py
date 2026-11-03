"""Joins the screenshot sandbox's network to this machine's, one named port at a time (#352).

The sandbox (sandbox.sh) has a network of its own: the app in it reaches no server running on
this machine. cap.py opens what the pictures need through unix sockets in the runtime folder,
which both sides share: Broadway's and WebKit's inspector's ports out to Chromium, and a stand-in
server's port (fake_model, fake-online) in to the app. Standard library only, as it also runs
inside the sandbox:

    python3 netbridge.py LISTEN TARGET [LISTEN TARGET ...]   # a number is a port on 127.0.0.1,
    python3 netbridge.py --self-test                         # anything else a unix socket
"""

import os, signal, socket, sys, tempfile, threading


def _sock(addr):
    return socket.socket(socket.AF_INET if isinstance(addr, int) else socket.AF_UNIX)


def _where(addr):
    return ("127.0.0.1", addr) if isinstance(addr, int) else addr


def _pump(src, dst):
    try:
        while data := src.recv(1 << 16):
            dst.sendall(data)
    except OSError:
        pass
    try:
        dst.shutdown(socket.SHUT_WR)
    except OSError:
        pass


def _serve(conn, target):
    out = _sock(target)
    try:
        out.connect(_where(target))
    except OSError:
        conn.close()
        out.close()
        return
    back = threading.Thread(target=_pump, args=(out, conn), daemon=True)
    back.start()
    _pump(conn, out)
    back.join()
    conn.close()
    out.close()


def bridge(listen, target):
    """Listen on `listen` and join each caller to `target` (a port on 127.0.0.1, or a unix
    socket's path). Returns the listening socket; `stop(it)` ends it."""
    s = _sock(listen)
    if isinstance(listen, int):
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(_where(listen))
    s.listen(64)

    def accept():
        while True:
            try:
                c, _ = s.accept()
            except OSError:
                return
            threading.Thread(target=_serve, args=(c, target), daemon=True).start()
    threading.Thread(target=accept, daemon=True).start()
    return s


def stop(s):
    try:
        s.shutdown(socket.SHUT_RDWR)  # wakes the accept() waiting on it
    except OSError:
        pass
    s.close()


def _addr(a):
    return int(a) if a.isdigit() else a


def self_test():
    """A port joined through a unix socket to another port carries both ways and its close."""
    with socket.socket() as srv, tempfile.TemporaryDirectory() as d:
        srv.bind(("127.0.0.1", 0))
        srv.listen()
        port = srv.getsockname()[1]
        path = os.path.join(d, "s")
        inner = bridge(path, port)
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            front = probe.getsockname()[1]
        outer = bridge(front, path)
        with socket.create_connection(("127.0.0.1", front), timeout=5) as c:
            c.sendall(b"ping")
            peer, _ = srv.accept()
            peer.settimeout(5)
            assert peer.recv(4) == b"ping"
            peer.sendall(b"pong")
            peer.close()
            got = b""
            while chunk := c.recv(16):
                got += chunk
            assert got == b"pong", got
        stop(outer)
        stop(inner)
    print("netbridge self-test: ok")


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["--self-test"]:
        self_test()
        sys.exit()
    if not args or len(args) % 2:
        sys.exit(__doc__)
    for listen, target in zip(args[::2], args[1::2]):
        bridge(_addr(listen), _addr(target))
    signal.pause()
