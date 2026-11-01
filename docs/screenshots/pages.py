"""The docs pages' pictures that need more than one screen reached: every Game family's board in
every Board theme, the GIF of a game against yourself, and the screens behind a tab or a button.

    DEMO_HOME=<dir> uv run --with websocket-client python pages.py [name ...]

With no name, it takes them all. Each run writes the demo home's config.toml for its picture
and removes it afterwards, so other pictures keep the default Settings. The engine picture
needs `fake-uci` (a stand-in UCI engine) in the demo home; see `engine()`.
"""

import json, os, sys
from pathlib import Path

from cap import HERE, SEEN, WIDTHS, Desktop, _env, gif, widths

FAMILIES = ["chess", "xiangqi", "shogi", "go", "draughts", "tafl", "backgammon", "ur", "senet"]
NAMES = {f: f.capitalize() for f in FAMILIES}
W1100 = (1100, 760)


def config(text):
    """The demo home's config.toml: `text`, or none."""
    p = Path(_env()["DEMO_HOME"]) / ".config/bragd/config.toml"
    p.parent.mkdir(parents=True, exist_ok=True)
    if text:
        p.write_text(text)
    else:
        p.unlink(missing_ok=True)


def js(d, body):
    return d.app(f"(() => {{ {body} }})()")


def click(d, text, scope="", exact=True, nth=0):
    """Click the `nth` button (or tab) whose text is `text`, inside `scope` when given."""
    test = f"t === {json.dumps(text)}" if exact else f"t.startsWith({json.dumps(text)})"
    r = js(d, f"""const bs = [...document.querySelectorAll({json.dumps((scope + ' ' if scope else '') + 'button')})]
        .filter(b => {{ const t = b.textContent.trim(); return {test}; }});
      if (!bs[{nth}]) return 'none'; bs[{nth}].click(); return 'ok';""")
    if r != "ok":
        raise RuntimeError(f"no button {text!r}")
    d.wait(0.6)


def nav(d, family, item="Play"):
    """The sidebar: open `family` and click its `item`."""
    name = NAMES[family]
    js(d, f"""const bs = [...document.querySelectorAll('nav button')];
      const i = bs.findIndex(b => b.classList.contains('family') && b.textContent.trim() === {json.dumps(name)});
      if (!bs[i + 1] || !bs[i + 1].classList.contains('item')) bs[i].click();""")
    d.wait(0.4)
    js(d, f"""const bs = [...document.querySelectorAll('nav button')];
      const i = bs.findIndex(b => b.classList.contains('family') && b.textContent.trim() === {json.dumps(name)});
      for (let j = i + 1; j < bs.length && bs[j].classList.contains('item'); j++)
        if (bs[j].textContent.trim() === {json.dumps(item)}) {{ bs[j].click(); return; }}""")
    d.wait(1.2)


def fold(d, family):
    """Close `family` in the sidebar, so the next picture shows only the Family it is about."""
    js(d, f"""const bs = [...document.querySelectorAll('nav button')];
      const i = bs.findIndex(b => b.classList.contains('family') && b.textContent.trim() === {json.dumps(NAMES[family])});
      if (bs[i + 1] && bs[i + 1].classList.contains('item')) bs[i].click();""")
    d.wait(0.3)


def select(d, label, value):
    """Choose `value` (the text it shows) in the list named `label` (a Picker's aria-label)."""
    r = js(d, f"""const b = document.querySelector('button[aria-haspopup=listbox][aria-label={json.dumps(label)}]');
      if (!b) return 'none'; b.click(); return 'ok';""")
    d.wait(0.4)
    if r == "ok":
        r = js(d, f"""const o = [...document.querySelectorAll('[role=option]')].find(o => o.querySelector('span').textContent.trim() === {json.dumps(value)});
          if (!o) return 'no option'; o.click(); return 'ok';""")
    if r != "ok":
        raise RuntimeError(f"select {label}: {r}")
    d.wait(0.8)


def cell(d, square):
    js(d, f"document.querySelector('rect.hit[data-square={json.dumps(square)}]').dispatchEvent(new MouseEvent('click', {{bubbles: true}}))")


def move(d, a, b, gap=0.35):
    cell(d, a)
    d.wait(gap)
    cell(d, b)
    d.wait(gap)


def boards():
    """board-<family>-<theme>.png: each Family's board in each Board theme; Match with Nord."""
    for theme, cfg in [("racing", ""), ("slate", '[look]\nboard_theme = "slate"\n'), ("walnut", '[look]\nboard_theme = "walnut"\n'),
                       ("match", '[look]\ntheme = "nord"\nboard_theme = "match"\n')]:
        config(cfg)
        try:
            with Desktop(size=W1100) as d:
                for prev, f in zip([None, *FAMILIES], FAMILIES):
                    nav(d, f)
                    # The sidebar keeps the current page's Family open: fold the one before.
                    if prev:
                        fold(d, prev)
                    d.shot(HERE / f"board-{f}-{theme}.png")
        finally:
            config("")


def selfplay():
    """play-chess-self.gif and .png: a game of chess played against yourself."""
    with Desktop(size=W1100) as d:
        nav(d, "chess")
        line = ["e2 e4", "e7 e5", "g1 f3", "b8 c6", "f1 c4", "g8 f6", "f3 g5", "d7 d5", "e4 d5"]
        frames = d.record(7.0, during=lambda: [d.wait(0.5)] + [move(d, *m.split(), gap=0.33) for m in line])
    gif(frames, HERE / "play-chess-self.gif", hold=0.4)


def engine():
    """play-engine-chess-1100.png: Play against an Engine (fake-uci in the demo home stands in
    for Stockfish), a few moves in, the opening named under the board."""
    config('[engines.paths]\nstockfish = "/home/demo/fake-uci"\n')
    try:
        with Desktop(size=W1100) as d:
            nav(d, "chess")
            select(d, "Opponent", "Against an engine")
            for a, b in (m.split() for m in ["e2 e4", "g1 f3", "f1 c4"]):
                move(d, a, b)
                d.wait(2.5)
            d.shot(HERE / "play-engine-chess-1100.png")
    finally:
        config("")


def review():
    """review-chess-390.png and -1100.png: a Game reviewed in Analyse (fake-uci in the demo home
    stands in for Stockfish, and calls 6...Qg5, off its line, a blunder), its Annotations listed
    and marked in the move list. position-store-390.png and -1100.png: Disk watch at the
    Position store's Export and Import."""
    config('[engines.paths]\nstockfish = "/home/demo/fake-uci"\n')
    try:
        def page(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            nav(d, "chess", "Analyse")
            for a, b in (m.split() for m in ["e2 e4", "e7 e5", "g1 f3", "b8 c6", "f1 c4", "d8 g5"]):
                move(d, a, b)
            click(d, "Review the game", "main")
            d.wait(5)
            js(d, "document.querySelector('main .notes')?.scrollIntoView({block: 'center'})")

        widths("review-chess", page)

        def knowledge(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            click(d, "Settings", "nav")
            click(d, "Knowledge", "main")
            d.wait(1.5)
            js(d, "[...document.querySelectorAll('main h3')].find(h => h.textContent.trim() === 'Position store')?.scrollIntoView({block: 'start'})")

        widths("position-store", knowledge)
    finally:
        config("")


def screens():
    """The screens behind a tab or a button."""
    with Desktop(size=W1100) as d:
        click(d, "What's new", "nav")
        d.shot(HERE / "whats-new-1100.png")
        click(d, "Settings", "nav")
        click(d, "Look", "main")
        d.shot(HERE / "settings-look-1100.png")
        click(d, "Privacy and updates", "main")
        d.shot(HERE / "settings-privacy-1100.png")
        click(d, "Help", "nav", exact=False)
        d.wait(1)
        d.shot(HERE / "help-1100.png")
        nav(d, "chess", "Opening books")
        d.shot(HERE / "books-1100.png")
        nav(d, "chess", "Databases")
        click(d, "Libraries", "main")
        js(d, "document.querySelector('main td button.link').click()")
        d.wait(1)
        click(d, "Duplicates", "main")
        d.wait(1)
        d.shot(HERE / "duplicates-1100.png")
        click(d, "New Slice", "main")
        d.wait(1)
        d.shot(HERE / "slice-editor-1100.png")
        click(d, "Sources", "main")
        click(d, "Options", "main")
        # A rule as a user might set it: both players rated 2000 or more, classical Games only.
        js(d, """const f = document.querySelector('main fieldset');
          const n = f.querySelector('input[type=number]'); n.value = '2000'; n.dispatchEvent(new Event('input', {bubbles: true}));
          const c = [...f.querySelectorAll('label')].find(l => l.textContent.trim() === 'Classical');
          c.querySelector('input').click();""")
        d.wait(1)
        d.shot(HERE / "strip-rule-1100.png")


def themes():
    """play-chess-nord-1100.png and look-win95-1100.png: a Theme and a Look other than the default."""
    for name, cfg in [("play-chess-nord-1100.png", '[look]\ntheme = "nord"\n'), ("look-win95-1100.png", '[look]\ntheme = "win95"\n')]:
        config(cfg)
        try:
            with Desktop(size=W1100) as d:
                nav(d, "chess")
                d.shot(HERE / name)
        finally:
            config("")


def help_():
    """help-390.png and help-1100.png: F1, whose features follow docs/README.md's order."""
    widths("help", lambda d: (d.keys("F1"), d.wait(1)))


def fill(d, label, value):
    """Type `value` into the field inside the label whose text starts with `label`."""
    r = js(d, f"""const l = [...document.querySelectorAll('main label')].find(l => l.textContent.trim().startsWith({json.dumps(label)}));
      const i = l && l.querySelector('input'); if (!i) return 'none';
      i.value = {json.dumps(value)}; i.dispatchEvent(new Event('input', {{bubbles: true}})); return 'ok';""")
    if r != "ok":
        raise RuntimeError(f"no field {label!r}")


def clubs():
    """clubs-390.png (the Clubs page before a Club is open) and clubs-1100.png (a Club made
    there, with members, a rating set by hand and its audit log)."""
    def page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        click(d, "Clubs", "nav")
    widths("clubs", page)
    d0 = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/clubs/Skovlunde Skakklub.bragd-club"
    try:
        with Desktop(size=W1100) as d:
            page(d)
            fill(d, "Club name", "Skovlunde Skakklub")
            fill(d, "Your name", "Mette Holm")
            click(d, "Create Club")
            for name, fide, nat, lichess in [("Bo Kjær", "1401234", "100200300", "bokjaer"), ("Anna Lind", "", "100200301", "")]:
                click(d, "Add a member")
                fill(d, "Name", name)
                fill(d, "FIDE ID", fide)
                fill(d, "National ID", nat)
                fill(d, "Federation", "DEN" if nat else "")
                fill(d, "Lichess", lichess)
                click(d, "Save")
            click(d, "Set rating", nth=0)
            fill(d, "New rating", "1640")
            fill(d, "Why", "Rated games at the summer open")
            click(d, "Save")
            d.shot(HERE / "clubs-1100.png")
    finally:
        for p in (d0, d0.with_name(d0.name + ".lock")):
            p.unlink(missing_ok=True)
        st = d0.parent.parent / "state.json"
        if st.exists():
            st.write_text(json.dumps({k: v for k, v in json.loads(st.read_text()).items() if k != "clubs"}))


def result(d, board, value):
    """Choose the result on `board` (from 0) of the Round shown first."""
    js(d, f"""const s = [...document.querySelectorAll('main select')].filter(s => (s.getAttribute('aria-label') || '').startsWith('Result of'))[{board}];
      s.value = {json.dumps(value)}; s.dispatchEvent(new Event('change', {{bubbles: true}}));""")
    d.wait(0.8)


def tournaments():
    """tournaments-1100.png (a Swiss on its Rounds tab: Round 2 paired, some results in),
    tournaments-390.png (its standings after Round 1) and tournaments-new-390/1100.png (the New
    Tournament form with Drop-in chosen: the system's one-line explanation and its standings)."""
    def club_page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        click(d, "Clubs", "nav")
    d0 = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/clubs/Skovlunde Skakklub.bragd-club"
    players = [("Bo Kjær", 2105), ("Anna Lind", 1980), ("Peter Dahl", 1875), ("Sara Holm", 1820), ("Jens Moe", 1760), ("Lise Berg", 1690), ("Karl Vang", 1610), ("Eva Ravn", 1540), ("Ole Friis", 1450)]
    results = ["white", "draw", "black", "white"]

    def new_form(d, width):
        """The New Tournament form with Drop-in chosen, scrolled into view."""
        js(d, """const s = [...document.querySelectorAll('main label')].find(l => l.textContent.trim().startsWith('Pairing system')).querySelector('select');
          s.value = 'drop_in'; s.dispatchEvent(new Event('change', {bubbles: true}));""")
        d.wait(0.5)
        js(d, "[...document.querySelectorAll('main h4')].find(h => h.textContent.trim() === 'New Tournament').scrollIntoView()")
        d.wait(0.5)
        d.shot(HERE / f"tournaments-new-{width}.png")
    try:
        with Desktop(size=W1100) as d:
            club_page(d)
            fill(d, "Club name", "Skovlunde Skakklub")
            fill(d, "Your name", "Mette Holm")
            click(d, "Create Club")
            new_form(d, 1100)
            js(d, """const s = [...document.querySelectorAll('main label')].find(l => l.textContent.trim().startsWith('Pairing system')).querySelector('select');
              s.value = 'dutch'; s.dispatchEvent(new Event('change', {bubbles: true}));""")
            fill(d, "Name", "Autumn Swiss 2026")
            fill(d, "Place", "Skovlunde")
            click(d, "Create Tournament")
            d.wait(1)
            for name, rating in players:
                fill(d, "Name", name)
                fill(d, "Rating", str(rating))
                click(d, "Add guest")
            click(d, "Rounds", "main")
            click(d, "Pair Round 1", "main")
            for k, r in enumerate(results):
                result(d, k, r)
            click(d, "Pair Round 2", "main")
            for k, r in enumerate(["draw", "white"]):
                result(d, k, r)
            js(d, "document.querySelector('main [role=tablist]').scrollIntoView()")
            d.wait(0.5)
            d.shot(HERE / "tournaments-1100.png")
        with Desktop(size=(390, 844)) as d:
            club_page(d)
            click(d, "Skovlunde Skakklub", "main")
            new_form(d, 390)
            click(d, "Autumn Swiss 2026", "main")
            click(d, "Standings", "main")
            js(d, "document.querySelector('main [role=tablist]').scrollIntoView()")
            d.wait(0.5)
            d.shot(HERE / "tournaments-390.png")
    finally:
        for p in (d0, d0.with_name(d0.name + ".lock")):
            p.unlink(missing_ok=True)
        st = d0.parent.parent / "state.json"
        if st.exists():
            st.write_text(json.dumps({k: v for k, v in json.loads(st.read_text()).items() if k != "clubs"}))


def leagues():
    """leagues-1100.png (a League of four Teams on its Rounds tab: Round 1 played) and
    leagues-390.png (its standings)."""
    def club_page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        click(d, "Clubs", "nav")
    d0 = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/clubs/Skovlunde Skakklub.bragd-club"
    teams = {"Skovlunde 1": ["Bo Kjær", "Anna Lind"], "Ballerup": ["Peter Dahl", "Sara Holm"], "Herlev": ["Jens Moe", "Lise Berg"], "Ishøj": ["Karl Vang", "Eva Ravn"]}
    try:
        with Desktop(size=W1100) as d:
            club_page(d)
            fill(d, "Club name", "Skovlunde Skakklub")
            fill(d, "Your name", "Mette Holm")
            click(d, "Create Club")
            fill(d, "League name", "Division 2")
            fill(d, "Boards per match", "2")
            click(d, "Create League")
            d.wait(1)
            for team, players in teams.items():
                fill(d, "Team name", team)
                for p in players:
                    js(d, f"""const i = document.querySelector('main input[aria-label="A guest\\'s name"]');
                      i.value = {json.dumps(p)}; i.dispatchEvent(new Event('input', {{bubbles: true}}));""")
                    d.wait(0.2)
                    click(d, "Add", "main", nth=1)
                click(d, "Add Team")
            click(d, "Rounds", "main")
            click(d, "Pair Round 1", "main")
            for k, r in enumerate(["white", "draw", "black", "black"]):
                result(d, k, r)
            js(d, "document.querySelector('main [role=tablist]').scrollIntoView()")
            d.wait(0.5)
            d.shot(HERE / "leagues-1100.png")
        with Desktop(size=(390, 844)) as d:
            club_page(d)
            click(d, "Skovlunde Skakklub", "main")
            click(d, "Division 2", "main")
            click(d, "Standings", "main")
            js(d, "document.querySelector('main [role=tablist]').scrollIntoView()")
            d.wait(0.5)
            d.shot(HERE / "leagues-390.png")
    finally:
        for p in (d0, d0.with_name(d0.name + ".lock")):
            p.unlink(missing_ok=True)
        st = d0.parent.parent / "state.json"
        if st.exists():
            st.write_text(json.dumps({k: v for k, v in json.loads(st.read_text()).items() if k != "clubs"}))


def matches():
    """matches-chess-390.png and matches-chess-1100.png: Chess → Engine matches, an SPRT match
    between two stand-ins (fake-uci as Stockfish and as Fruit) played a few Games in the Helper."""
    config('[engines.paths]\nstockfish = "/home/demo/fake-uci"\nfruit = "/home/demo/fake-uci"\n')
    lib = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/libraries/matches.bragd"
    try:
        with Desktop(size=W1100) as d:
            nav(d, "chess", "Engine matches")
            js(d, """for (const l of document.querySelectorAll('main fieldset label.check')) {
                const t = l.textContent.trim(); const i = l.querySelector('input');
                if (t === 'Stockfish' || t === 'Fruit' || t === 'SPRT') i.click(); }""")
            d.wait(0.5)
            fill(d, "Time per move (ms)", "100")
            click(d, "Start match", "main")
            d.wait(12)
        def page(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            nav(d, "chess", "Engine matches")
            d.wait(1)
            js(d, "document.querySelector('main h2:nth-of-type(2)').scrollIntoView()")
            d.wait(0.5)
        widths("matches-chess", page)
    finally:
        config("")
        for p in (lib, lib.with_name(lib.name + "-wal"), lib.with_name(lib.name + "-shm")):
            p.unlink(missing_ok=True)


def wizard():
    """wizard-<step>-390.png and -1100.png: the install wizard's six steps on a first start, and
    whats-new-card-390.png and -1100.png: the first tour card after an upgrade."""
    state = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/state.json"
    for w, h in WIDTHS.items():
        with Desktop(size=(w, h), state={}) as d:
            d.wait(2)
            for i, step in enumerate(["welcome", "games", "engines", "knowledge", "online", "helper"]):
                if i:
                    click(d, "Next", ".wizard footer")
                d.shot(HERE / f"wizard-{step}-{w}.png")
        with Desktop(size=(w, h), state=dict(SEEN, seen_version="0.1.0")) as d:
            d.wait(2)
            d.shot(HERE / f"whats-new-card-{w}.png")
    # The next picture starts as a demo home does, having seen the wizard and every card.
    state.unlink(missing_ok=True)


def players():
    """A player's summary over the demo Library, found through Find a player."""
    def page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        nav(d, "chess", "Databases")
        click(d, "Libraries", "main")
        js(d, "document.querySelector('main td button.link').click()")
        d.wait(1)
        js(d, """const i = document.querySelector('main input[type=search]');
          i.value = 'hansen'; i.dispatchEvent(new Event('input', {bubbles: true}));
          i.form.requestSubmit();""")
        d.wait(1.5)
    widths("player-summary", page)


# A Game with a comment in German, for Translate (model()).
COMMENTS_PGN = """[Event "Demo"]
[Site "Demo"]
[Date "2026.01.03"]
[White "Weiß"]
[Black "Schwarz"]
[Result "*"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 {Die Spanische Partie. Weiß greift den Springer an, der den Bauern auf e5 verteidigt.} a6 4. Ba4 Nf6 *
"""


def fake_model():
    """A stand-in language model on 127.0.0.1, in a thread: its model list names one model, and
    every chat answer is the English of COMMENTS_PGN's comment. Stop it with `.shutdown()`."""
    import http.server, threading

    class Fake(http.server.BaseHTTPRequestHandler):
        def answer(self, body):
            data = json.dumps(body).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            self.answer({"object": "list", "data": [{"id": "demo-translator", "object": "model"}]})

        def do_POST(self):
            self.rfile.read(int(self.headers.get("Content-Length", 0)))
            text = "The Spanish Game. White attacks the knight that defends the pawn on e5."
            self.answer({"choices": [{"index": 0, "message": {"role": "assistant", "content": text}}]})

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fake)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def model():
    """language-model-390.png and -1100.png: Settings → Language model with the model server
    running on this machine, and translate-comment-390.png and -1100.png: a German comment
    translated in replay, and translate-library-*.png: the same comment in a Library Game. The model is a stand-in (fake_model) unless MODEL_URL and MODEL_NAME
    name a real server on the host; one translation is asked, then kept in the demo home. The Foreign database
    is opened through a stand-in for Coxswain's picker, put beside bragd-gui for the run."""
    fake = None
    if os.environ.get("MODEL_URL"):
        url, name = os.environ["MODEL_URL"], os.environ.get("MODEL_NAME", "")
    else:
        fake = fake_model()
        url, name = f"http://127.0.0.1:{fake.server_port}/v1", "demo-translator"
    home = Path(_env()["DEMO_HOME"])
    (home / "Games/comments.pgn").write_text(COMMENTS_PGN)
    picker = HERE.parent.parent / "target/release/coxswain-gui"
    picker.write_text('#!/bin/sh\ncase "$1" in --help) echo "--pick";; *) echo /home/demo/Games/comments.pgn;; esac\n')
    picker.chmod(0o755)
    config(f'[look]\nfile_picker = "coxswain"\n\n[model]\nurl = "{url}"\nname = "{name}"\n')
    try:
        def settings(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            click(d, "Settings", "nav")
            click(d, "Language model", "main")
            d.wait(2)

        widths("language-model", settings)

        def comment(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            nav(d, "chess", "Databases")
            click(d, "Open a file…", "main")
            d.wait(1.5)
            js(d, "document.querySelector('main .body .row').click()")
            d.wait(1.5)
            js(d, "document.querySelectorAll('main .moves button')[4].click()")
            d.wait(1)
            js(d, "document.querySelector('main button.translate')?.click()")
            # A local model may take a while; the second width finds it kept.
            for _ in range(60):
                if js(d, "return !!document.querySelector('main .machine')"):
                    break
                d.wait(1)
            js(d, "document.querySelector('main .comment')?.scrollIntoView({block: 'center'})")

        widths("translate-comment", comment)

        def library(d):
            unfold(d)
            nav(d, "chess", "Databases")
            click(d, "Libraries", "main")
            js(d, "document.querySelector('main td button.link').click()")
            d.wait(1.5)
            js(d, "[...document.querySelectorAll('main .row')].find(r => r.textContent.includes('Schwarz'))?.click()")
            d.wait(1.5)
            js(d, "document.querySelectorAll('main .moves button')[4].click()")
            d.wait(1)
            js(d, "document.querySelector('main button.translate')?.click()")
            for _ in range(30):
                if js(d, "return !!document.querySelector('main .machine')"):
                    break
                d.wait(1)
            js(d, "document.querySelector('main .comment')?.scrollIntoView({block: 'center'})")

        widths("translate-library", library, out=Path(os.environ.get("CAP_OUT", HERE)))
    finally:
        config("")
        picker.unlink(missing_ok=True)
        if fake:
            fake.shutdown()


def background():
    """background-analysis-390.png and -1100.png: a saved Slice with Analyse in background open
    (fake-uci as Stockfish); background-jobs-*.png: the job it queued on the Jobs page, and
    store-*.png: Disk watch's Position store with what it kept."""
    config('[engines.paths]\nstockfish = "/home/demo/fake-uci"\n[analysis]\nopen_games = false\n')
    store = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/positions.sqlite"
    # The Helper's jobs of an earlier run would show again on the Jobs page.
    state = Path(_env()["DEMO_HOME"]) / ".local/share/bragd/helper/state.json"

    def slice_page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        nav(d, "chess", "Databases")
        click(d, "Libraries", "main")
        js(d, "document.querySelector('main td button.link').click()")
        d.wait(1)
        js(d, "const t = [...document.querySelectorAll('main nav.tabs button')]; t[t.length - 1].click()")
        d.wait(1)
        click(d, "Analyse in background", "main")
        d.wait(1.5)
        js(d, "document.querySelector('main .analyse').scrollIntoView()")
        d.wait(0.5)

    def jobs_page(d):
        slice_page(d)
        click(d, "Queue", "main .analyse")
        d.wait(6)
        js(d, "document.querySelector('button.helper').click()")
        d.wait(1.5)
        js(d, "const h = [...document.querySelectorAll('main h2')].find(h => h.textContent.trim() === 'Background analysis'); h && h.scrollIntoView()")
        d.wait(0.5)

    def store_page(d):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        click(d, "Settings", "nav")
        click(d, "Knowledge", "main")
        d.wait(2)
        js(d, "const h = [...document.querySelectorAll('main h3')].find(h => h.textContent.trim() === 'Position store'); h && h.scrollIntoView()")
        d.wait(0.5)

    try:
        widths("background-analysis", slice_page)
        for w, h in WIDTHS.items():
            state.unlink(missing_ok=True)
            with Desktop(size=(w, h)) as d:
                jobs_page(d)
                d.shot(HERE / f"background-jobs-{w}.png")
        widths("store", store_page)
    finally:
        config("")
        for p in (state, store, store.with_name(store.name + "-wal"), store.with_name(store.name + "-shm")):
            p.unlink(missing_ok=True)


def peers():
    """peers-390.png and peers-1100.png: Settings → Peers on, a code shown, and one paired
    machine that is not reachable now (at 192.0.2.10, an address kept for documentation). The
    sandbox's Helper listens on the host's home network while it runs, under the name demo."""
    home = Path(_env()["DEMO_HOME"])
    folder = home / ".local/share/bragd/helper/peers"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "peers.json").write_text(json.dumps([{"key": "5e" * 32, "name": "laptop", "addr": "192.0.2.10:47247", "paired": 1793000000, "seen": 1793400000, "version": "0.5.0"}]))
    config("[peers]\non = true\n")
    try:
        def page(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            click(d, "Settings", "nav")
            click(d, "Peers", "main")
            d.wait(4)
            click(d, "Show a code", "main")
            d.wait(1)
            # The host's own addresses stay out of the docs: an example address in their place.
            js(d, "const s = document.querySelector('main .peers .state'); if (s) s.textContent = s.textContent.replace(/listening on .*$/, 'listening on 192.168.1.20:47247.');")

        widths("peers", page)
    finally:
        config("")
        (folder / "peers.json").unlink(missing_ok=True)


def sharing():
    """peers-sharing-390.png and peers-sharing-1100.png: Settings → Peers with a real paired
    machine sharing: its sync and what it works on, and the shared queue. SHARING_FROM names a
    home whose Helper was paired with another machine that is running now (the two-machine test
    in `docs/peers/sharing.md`'s PR): its key, its Peers, its Position store and its jobs are
    copied into the demo home, so the sandbox's Helper is that machine, and removed afterwards."""
    import shutil
    home = Path(_env()["DEMO_HOME"])
    src = Path(os.environ["SHARING_FROM"]) / ".local/share/bragd"
    data = home / ".local/share/bragd"
    copied = [data / "helper/peers", data / "positions.sqlite", data / "helper/state.json"]
    shutil.copytree(src / "helper/peers", data / "helper/peers", dirs_exist_ok=True)
    shutil.copy(src / "positions.sqlite", data / "positions.sqlite")
    shutil.copy(src / "helper/state.json", data / "helper/state.json")
    # Background analysis waits outside its hours here, so the job stays for the other machine.
    config("[peers]\non = true\n[analysis]\nfrom_hour = 3\nuntil_hour = 4\n")
    try:
        def page(d):
            js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
            d.wait(0.4)
            click(d, "Settings", "nav")
            click(d, "Peers", "main")
            # The other machine asks for work once a minute: time for it to take the job.
            d.wait(70)
            click(d, "Sync now", "main")
            d.wait(8)
            js(d, "const s = document.querySelector('main .peers .state'); if (s) s.textContent = s.textContent.replace(/listening on .*$/, 'listening on 192.168.1.20:47247.');")
            # The other machine's address too.
            js(d, "document.querySelectorAll('main .peers .peer small').forEach(s => { if (/^[0-9.:]+$/.test(s.textContent.trim())) s.textContent = '192.168.1.30:47247'; });")
            js(d, "const h = [...document.querySelectorAll('main h3')].find(h => h.textContent.trim() === 'Paired machines'); h && h.scrollIntoView()")
            d.wait(0.5)

        widths("peers-sharing", page)
    finally:
        config("")
        shutil.rmtree(copied[0], ignore_errors=True)
        for p in copied[1:]:
            p.unlink(missing_ok=True)


def online():
    """Play online against the fake servers (`fake-online`, built by `cargo build --release -p
    bragd-online --features fake --example fake-online`), never the real ones. Each fake runs on
    this machine, started and stopped here by its own PID; the app is pointed at it by
    BRAGD_<SERVER>_URL, and BRAGD_<SERVER>_TOKEN stands in for the keychain the sandbox does not
    have. Lichess: online-sign-in, online-lobby and online-game; Lishogi: online-lishogi-sign-in
    and online-lishogi; OGS: online-ogs-sign-in and online-ogs; each -390.png and -1100.png."""
    import subprocess
    fake = HERE.parent.parent / "target/release/examples/fake-online"

    def open_page(d, family, server):
        js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
        d.wait(0.4)
        nav(d, family, "Play")
        click(d, f"Play on {server}", "main")
        d.wait(2)

    def say(d):
        js(d, "const i = document.querySelector('main .say input'); i.value = 'Hello, have a good game!'; i.dispatchEvent(new Event('input', {bubbles: true}));")
        d.wait(0.3)
        click(d, "Send", "main")
        d.wait(1.5)

    def lichess(d):
        click(d, "Accept", "main")
        d.wait(2)
        for a, b in [("e2", "e4"), ("g1", "f3")]:
            move(d, a, b)
            d.wait(1.5)
        say(d)

    def lishogi(d):
        click(d, "Accept", "main")
        d.wait(2)
        for a, b in [("7g", "7f"), ("2g", "2f")]:
            move(d, a, b)
            d.wait(1.5)

    def ogs(d):
        click(d, "Accept", "main")
        d.wait(2.5)
        cell(d, "d4")
        d.wait(1.5)

    servers = [("lichess", "chess", "Lichess", "lip_fake", "online-sign-in", lichess, "online-game"),
               ("lishogi", "shogi", "Lishogi", "lip_fake", "online-lishogi-sign-in", lishogi, "online-lishogi"),
               ("ogs", "go", "OGS", "ogs_fake", "online-ogs-sign-in", ogs, "online-ogs")]
    for key, family, server, token, sign_in, play, game in servers:
        var = f"BRAGD_{key.upper()}"
        for w, h in WIDTHS.items():
            p = subprocess.Popen([str(fake), key], stdout=subprocess.PIPE, text=True)
            try:
                url = p.stdout.readline().strip()
                with Desktop(size=(w, h), env={f"{var}_URL": url}) as d:
                    open_page(d, family, server)
                    d.shot(HERE / f"{sign_in}-{w}.png")
                config("[online]\nchat = true\n")
                with Desktop(size=(w, h), env={f"{var}_URL": url, f"{var}_TOKEN": token}) as d:
                    open_page(d, family, server)
                    if key == "lichess":
                        d.shot(HERE / f"online-lobby-{w}.png")
                    play(d)
                    d.shot(HERE / f"{game}-{w}.png")
            finally:
                config("")
                p.terminate()
                p.wait(5)


def sources():
    """databases-sources-390.png and -1100.png: Chess → Databases → Sources, TWIC first and the
    Sources not fetched yet greyed out, with their size and no schedule."""
    def page(d):
        unfold(d)
        nav(d, "chess", "Databases")
        click(d, "Sources", "main")
        d.wait(1.5)
    widths("databases-sources", page)


# The stand-in engines of the demo home, for every Family with an Engine but draughts.
ENGINES = '[engines.paths]\nstockfish = "/home/demo/fake-uci"\nkatago = "/home/demo/fake-engine"\nfairy-stockfish = "/home/demo/fake-engine"\ngnubg = "/home/demo/fake-engine"\n'
# A few moves to play in Analyse, and a player of the demo Library, per Family. A backgammon
# turn is a throw and the engine's first line.
LINES = {"chess": ["e2 e4", "e7 e5", "g1 f3", "b8 c6", "f1 c4", "d8 g5"], "go": ["e5", "c3", "g7", "g3", "c7", "e3", "d6"],
         "shogi": ["7g 7f", "3c 3d", "2g 2f", "8c 8d", "2f 2e"], "backgammon": ["throw"] * 4}
PLAYERS = {"chess": "hansen", "go": "sato", "shogi": "mori", "backgammon": "vik"}


def unfold(d):
    js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
    d.wait(0.4)


def stop(d):
    """Stop the engine on the Analyse page, when it runs."""
    js(d, "[...document.querySelectorAll('main button')].find(b => b.textContent.trim() === 'Stop')?.click()")
    d.wait(0.5)


def play_line(d, family):
    """Play LINES[family] on the Analyse page."""
    for m in LINES[family]:
        if m == "throw":
            click(d, "Throw", "main")
            d.wait(0.6)
            click(d, "Analyse", "main")
            d.wait(2.5)
            js(d, "document.querySelector('main .lines button.mv')?.click()")
            stop(d)
        else:
            for sq in m.split():
                cell(d, sq)
                d.wait(0.35)
    d.wait(1)


def families(only=None):
    """The 0.4.0 screens for chess, go, shogi and backgammon, each at 390 and 1100 px, into
    $CAP_OUT (default: here): analyse-stored-<family> (Analyse showing a line from the Position
    store, which an earlier run put there), review-<family> (Review with its Annotations and the
    move list's marks), slice-analyse-<family> (a Slice's Analyse in background), jobs-<family>
    (the Jobs page with the job it queued), player-<family> (Find a player), and once
    store-table (Disk watch's Position store). `only` ("go" or "chess"): every other Family
    turned off, and the start, Databases, Settings → Knowledge and F1 pictures as only-<family>-*."""
    out = Path(os.environ.get("CAP_OUT", HERE))
    home = Path(_env()["DEMO_HOME"]) / ".local/share/bragd"
    off = "".join(f"{f} = false\n" for f in FAMILIES if only and f != only)
    config(ENGINES + "[analysis]\nopen_games = false\n" + (f"[families.family]\n{off}" if off else ""))
    try:
        if only:
            def start(d):
                unfold(d)
                nav(d, only, "Analyse")
            widths(f"only-{only}-start", start, out=out)

            def library(d):
                unfold(d)
                nav(d, only, "Databases")
                click(d, "Libraries", "main")
                js(d, "document.querySelector('main td button.link').click()")
                d.wait(1.5)
            widths(f"only-{only}-library", library, out=out)

            def knowledge(d):
                unfold(d)
                click(d, "Settings", "nav")
                click(d, "Knowledge", "main")
                d.wait(1.5)
            widths(f"only-{only}-knowledge", knowledge, out=out)
            widths(f"only-{only}-help", lambda d: (d.keys("F1"), d.wait(1)), out=out)
            return
        for f in ["chess", "go", "shogi", "backgammon"]:
            # Analysed once, so the next start finds the line in the Position store.
            with Desktop(size=W1100) as d:
                nav(d, f, "Analyse")
                play_line(d, f)
                click(d, "Analyse", "main")
                d.wait(4)
                stop(d)

            def stored(d):
                unfold(d)
                nav(d, f, "Analyse")
                play_line(d, f)
                click(d, "Analyse", "main")
                d.wait(1.5)
            widths(f"analyse-stored-{f}", stored, out=out)

            def review(d):
                stored(d)
                click(d, "Review the game", "main")
                d.wait(6)
                js(d, "document.querySelector('main .notes')?.scrollIntoView({block: 'center'})")
            # Backgammon has no Review yet (docs/engines/review.md).
            if f != "backgammon":
                widths(f"review-{f}", review, out=out)

            def slice_page(d):
                unfold(d)
                nav(d, f, "Databases")
                click(d, "Libraries", "main")
                js(d, "document.querySelector('main td button.link').click()")
                d.wait(1)
                # The Family's own Slice: "White wins" for chess, "Go club" and so on.
                click(d, "White wins" if f == "chess" else f"{NAMES[f]} club", "main")
                d.wait(1)
                click(d, "Analyse in background", "main")
                d.wait(1.5)
                js(d, "document.querySelector('main .analyse')?.scrollIntoView()")
            widths(f"slice-analyse-{f}", slice_page, out=out)

            def jobs(d):
                slice_page(d)
                click(d, "Queue", "main .analyse")
                d.wait(5)
                js(d, "document.querySelector('button.helper').click()")
                d.wait(1.5)
                js(d, "const h = [...document.querySelectorAll('main h2')].find(h => h.textContent.trim() === 'Background analysis'); h && h.scrollIntoView()")
            # A slow stand-in, so the job is seen running; and no jobs of an earlier run.
            slow = Path(_env()["DEMO_HOME"]) / "fake-engine.slow"
            slow.touch()
            try:
                for w, h in WIDTHS.items():
                    (home / "helper/state.json").unlink(missing_ok=True)
                    with Desktop(size=(w, h)) as d:
                        jobs(d)
                        d.shot(out / f"jobs-{f}-{w}.png")
            finally:
                slow.unlink()

            def player(d):
                unfold(d)
                nav(d, f, "Databases")
                click(d, "Libraries", "main")
                js(d, "document.querySelector('main td button.link').click()")
                d.wait(1)
                js(d, f"""const i = document.querySelector('main input[type=search]');
                  i.value = {json.dumps(PLAYERS[f])}; i.dispatchEvent(new Event('input', {{bubbles: true}}));
                  i.form.requestSubmit();""")
                d.wait(1.5)
            widths(f"player-{f}", player, out=out)

        def store(d):
            unfold(d)
            click(d, "Settings", "nav")
            click(d, "Knowledge", "main")
            d.wait(2)
            js(d, "const h = [...document.querySelectorAll('main h3')].find(h => h.textContent.trim() === 'Position store'); h && h.scrollIntoView()")
        widths("store-table", store, out=out)
    finally:
        config("")
        for p in (home / "helper/state.json", home / "positions.sqlite", home / "positions.sqlite-wal", home / "positions.sqlite-shm"):
            p.unlink(missing_ok=True)


ALL = {"matches": matches, "clubs": clubs, "tournaments": tournaments, "leagues": leagues, "wizard": wizard, "help": help_, "boards": boards, "selfplay": selfplay, "engine": engine, "review": review, "screens": screens, "themes": themes, "players": players, "model": model, "background": background, "peers": peers, "sharing": sharing, "online": online, "families": families, "sources": sources,
       "only-go": lambda: families("go"), "only-chess": lambda: families("chess")}

if __name__ == "__main__":
    for n in sys.argv[1:] or ALL:
        ALL[n]()
