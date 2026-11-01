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


ALL = {"matches": matches, "clubs": clubs, "tournaments": tournaments, "leagues": leagues, "wizard": wizard, "help": help_, "boards": boards, "selfplay": selfplay, "engine": engine, "screens": screens, "themes": themes, "players": players}

if __name__ == "__main__":
    for n in sys.argv[1:] or ALL:
        ALL[n]()
