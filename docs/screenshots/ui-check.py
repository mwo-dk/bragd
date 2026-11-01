# /// script
# requires-python = ">=3.10"
# dependencies = ["websocket-client"]
# ///
"""The UI check: every main screen of Bragd, at 390 and 1100 px wide, fails on

  - text that does not fit: a button, label, list item, cell or heading whose text is wider
    than its box, or any element that cuts its text off (overflow hidden, an ellipsis), and a
    page wider than the window;
  - axe-core's rules on contrast, names and labels, and keyboard focus (axe-core is a
    devDependency of gui/, read from gui/node_modules: nothing is fetched);
  - a board that does not fit the window: at 1100 × 700 every Family's whole board shows.

It runs the release build in the screenshot sandbox (cap.py: a fake home, BRAGD_OFFLINE=1, GTK
Broadway in a headless Chromium) and checks the app's own page through WebKit's inspector.

    uv run --with websocket-client python docs/screenshots/ui-check.py [--shots DIR]

`--shots DIR` also saves a picture of every screen checked. Exit 1 lists every fault, by screen
and width. Needs what cap.py needs, and `npm ci` in gui/.
"""

import json, sys, time
from pathlib import Path

from cap import HERE, SEEN, Desktop
from pages import FAMILIES, NAMES, js

AXE = HERE.parent.parent / "gui/node_modules/axe-core/axe.min.js"

# The axe-core rules that are about contrast, names and labels, and the keyboard.
RULES = ["color-contrast", "label", "button-name", "link-name", "select-name", "input-button-name", "aria-input-field-name",
         "aria-toggle-field-name", "aria-command-name", "image-alt", "svg-img-alt", "frame-title", "document-title",
         "tabindex", "nested-interactive", "aria-hidden-focus", "scrollable-region-focusable", "focus-order-semantics",
         "aria-allowed-attr", "aria-required-attr", "aria-valid-attr-value", "aria-roles", "duplicate-id-aria", "label-title-only"]

# Text that does not fit, found in the page. A box that scrolls its content (overflow auto or
# scroll) is not a fault: it is meant to. One pixel is rounding.
OVERFLOW = r"""(() => {
  const out = [];
  const seen = (e) => { const s = getComputedStyle(e); const r = e.getBoundingClientRect();
    return s.visibility !== 'hidden' && s.display !== 'none' && r.width > 0 && r.height > 0; };
  const name = (e) => (e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') + (e.classList.length ? '.' + [...e.classList].join('.') : '')
    + ' "' + (e.innerText || e.value || e.getAttribute('aria-label') || '').trim().replace(/\s+/g, ' ').slice(0, 60) + '"');
  const scrolls = (s) => /(auto|scroll)/.test(s.overflowX);
  for (const e of document.querySelectorAll('body *')) {
    if (!seen(e) || e.closest('svg') || e.closest('.splash')) continue;
    const s = getComputedStyle(e);
    if (scrolls(s)) continue;
    const text = (e.innerText || '').trim();
    const over = e.scrollWidth - e.clientWidth > 1;
    const cuts = /(hidden|clip)/.test(s.overflowX) || s.textOverflow === 'ellipsis';
    const boxy = /^(BUTTON|LABEL|SELECT|INPUT|TH|TD|LI|A|SUMMARY|H1|H2|H3|H4|OPTION)$/.test(e.tagName);
    if (text && over && (cuts || boxy)) out.push('text does not fit: ' + name(e) + ` (${e.scrollWidth} > ${e.clientWidth} px)`);
    const r = e.getBoundingClientRect();
    if (text && r.right > innerWidth + 1 && !e.closest('[class*="scroll"], .page, nav')) out.push('beyond the window: ' + name(e));
  }
  const page = document.querySelector('main.page');
  if (page && page.scrollWidth - page.clientWidth > 1) out.push(`the page scrolls sideways (${page.scrollWidth} > ${page.clientWidth} px)`);
  if (document.documentElement.scrollWidth > innerWidth + 1) out.push('the window scrolls sideways');
  return out;
})()"""

# WebKit's inspector does not wait for a promise: the result is left in the page and fetched.
AXE_RUN = """(() => {
  window.__axeOut = undefined;
  axe.run(document, { runOnly: { type: 'rule', values: %s }, resultTypes: ['violations'] }).then(r => {
    window.__axeOut = r.violations.flatMap(v => v.nodes.map(n => v.id + ': ' + v.help + ' — ' + n.target.join(' ') + (n.failureSummary ? ' (' + n.failureSummary.split('\\n').slice(1).join(' ').trim().slice(0, 160) + ')' : '')));
  }, e => (window.__axeOut = ['axe-core failed: ' + e]));
  return 0;
})()"""


def run_axe(d, timeout=120):
    """axe-core's faults on the page now."""
    d.app(AXE_RUN % json.dumps(RULES))
    end = time.time() + timeout
    while time.time() < end:
        out = d.app("window.__axeOut === undefined ? null : JSON.stringify(window.__axeOut)")
        if out is not None:
            return json.loads(out)
        time.sleep(0.3)
    raise TimeoutError("axe-core did not finish")

# The whole board inside the window, with its trays.
BOARD = r"""(() => {
  const b = document.querySelector('.board-side'); if (!b) return 'no board';
  const r = b.getBoundingClientRect();
  return r.bottom <= innerHeight + 1 ? '' : `the board ends at ${Math.round(r.bottom)} px, below the window (${innerHeight} px)`;
})()"""


def load_axe(d, source):
    """axe-core into the app's page. The inspector drops a message that large, so it comes in
    pieces, put together and run there; the inspector's own evaluation is not held by the page's
    content security policy."""
    js(d, "window.__axe = []")
    step = 16_000
    for i in range(0, len(source), step):
        d.app(f"window.__axe.push({json.dumps(source[i:i + step])}), 0")
    if d.app("(0, eval)(window.__axe.join('')), delete window.__axe, typeof axe") != "object":
        raise RuntimeError("axe-core did not load in the app's page")


def unfold(d):
    js(d, "const f = document.querySelector('nav button.fold'); if (f && f.textContent.trim() === '»') f.click();")
    d.wait(0.4)


def item(d, family, name):
    """The sidebar's `name` under `family`."""
    unfold(d)
    label = json.dumps(NAMES[family])
    js(d, f"""const bs = [...document.querySelectorAll('nav button')];
      const i = bs.findIndex(b => b.classList.contains('family') && b.textContent.trim() === {label});
      if (!bs[i + 1] || !bs[i + 1].classList.contains('item')) bs[i].click();""")
    d.wait(0.3)
    r = js(d, f"""const bs = [...document.querySelectorAll('nav button')];
      const i = bs.findIndex(b => b.classList.contains('family') && b.textContent.trim() === {label});
      for (let j = i + 1; j < bs.length && bs[j].classList.contains('item'); j++)
        if (bs[j].textContent.trim() === {json.dumps(name)}) {{ bs[j].click(); return 'ok'; }}
      return 'none';""")
    if r != "ok":
        raise RuntimeError(f"no sidebar item {family} → {name}")
    d.wait(1.2)


def bottom(d, name):
    """Settings, Help or What's new, at the sidebar's foot."""
    unfold(d)
    r = js(d, f"""const b = [...document.querySelectorAll('nav button')].find(b => b.textContent.trim().startsWith({json.dumps(name)}));
      if (!b) return 'none'; b.click(); return 'ok';""")
    if r != "ok":
        raise RuntimeError(f"no sidebar button {name}")
    d.wait(1)


def tab(d, name):
    r = js(d, f"""const b = [...document.querySelectorAll('main button')].find(b => b.textContent.trim() === {json.dumps(name)});
      if (!b) return 'none'; b.click(); return 'ok';""")
    if r != "ok":
        raise RuntimeError(f"no button {name}")
    d.wait(1)


AREAS = ["Overview", "Game families", "Look", "Engines", "Knowledge", "Online", "Privacy and updates", "Language model", "Peers"]


def screens():
    """(name, how to reach it) for every main screen."""
    out = []
    for f in FAMILIES:
        out.append((f"play-{f}", lambda d, f=f: item(d, f, "Play")))
        out.append((f"rules-{f}", lambda d, f=f: item(d, f, "Rules")))
    for f in ["chess", "xiangqi", "go", "backgammon"]:
        out.append((f"engines-{f}", lambda d, f=f: item(d, f, "Engines")))
    out += [
        ("analyse-chess", lambda d: item(d, "chess", "Analyse")),
        # Signed out: no server is asked (the sandbox has no keychain and no token).
        ("online-chess", lambda d: (item(d, "chess", "Play"), tab(d, "Play on Lichess"))),
        ("online-shogi", lambda d: (item(d, "shogi", "Play"), tab(d, "Play on Lishogi"))),
        # OGS before Bragd is registered with it: the notice and the link instead of the sign-in.
        ("online-go", lambda d: (item(d, "go", "Play"), tab(d, "Play on OGS"))),
        ("databases", lambda d: item(d, "chess", "Databases")),
        ("libraries", lambda d: (item(d, "chess", "Databases"), tab(d, "Libraries"))),
        ("sources", lambda d: (item(d, "chess", "Databases"), tab(d, "Sources"))),
        ("tablebases", lambda d: item(d, "chess", "Tablebases")),
        ("books", lambda d: item(d, "chess", "Opening books")),
        ("clubs", lambda d: bottom(d, "Clubs")),
        ("help", lambda d: bottom(d, "Help")),
        ("whats-new", lambda d: bottom(d, "What's new")),
    ]
    for a in AREAS:
        out.append((f"settings-{a.split()[0].lower()}", lambda d, a=a: (bottom(d, "Settings"), tab(d, a))))
    return out


# Faults put into the page on purpose, which each check must find: a cut-off button, grey on
# grey, and a button with no name.
PLANTED = r"""(() => {
  const box = document.createElement('div');
  box.id = 'ui-check-planted';
  box.innerHTML = '<button style="width:3rem;overflow:hidden;white-space:nowrap">A button far too long for its box</button>'
    + '<p style="color:#bbb;background:#ccc">Grey on grey</p><button></button>';
  document.querySelector('main.page').prepend(box);
  return 0;
})()"""


def self_test():
    """The checks find faults: each planted one is reported, and goes with it."""
    with Desktop(size=(1100, 760), state=SEEN) as d:
        load_axe(d, AXE.read_text())
        js(d, f"return {PLANTED}")
        over = js(d, f"return {OVERFLOW}") or []
        found = run_axe(d)
        js(d, "document.getElementById('ui-check-planted').remove()")
    misses = [what for what, hit in [("the cut-off button", any("far too long" in x for x in over)),
                                     ("grey on grey", any(x.startswith("color-contrast") and "planted" in x for x in found)),
                                     ("the nameless button", any(x.startswith("button-name") and "planted" in x for x in found))] if not hit]
    for m in misses:
        print("SELF-TEST MISSED", m)
    if misses:
        print("found:", over, found)
    print(f"ui check self-test: {'ok' if not misses else 'failed'}")
    return not misses


WIZARD = ["welcome", "families", "engines", "knowledge", "online", "helper"]


def look(d, name, w, h, shots):
    """The faults of the screen shown now."""
    if shots:
        d.shot(Path(shots) / f"{name}-{w}x{h}.png")
    found = list(js(d, f"return {OVERFLOW}") or [])
    found += run_axe(d)
    if h <= 700 and name.startswith("play-"):
        if b := js(d, f"return {BOARD}"):
            found.append(b)
    print(f"{name} at {w}×{h}: {'ok' if not found else f'{len(found)} faults'}", flush=True)
    return [f"{name} at {w}×{h}: {x}" for x in found]


def check(sizes, shots=None, only=None):
    axe = AXE.read_text()
    faults = []
    for w, h in sizes:
        # A home that has seen the wizard, whatever the wizard's run below left in it.
        with Desktop(size=(w, h), state=SEEN) as d:
            load_axe(d, axe)
            for name, reach in screens():
                if only and name not in only:
                    continue
                try:
                    reach(d)
                except Exception as e:  # a screen that cannot be reached is a fault too
                    faults.append(f"{name} at {w}×{h}: cannot reach it: {e}")
                    continue
                faults += look(d, name, w, h, shots)
        # The install wizard of a first start, each of its steps.
        if only and not any(o.startswith("wizard") for o in only):
            continue
        with Desktop(size=(w, h), state={}) as d:
            load_axe(d, axe)
            for i, step in enumerate(WIZARD):
                if i and js(d, """const b = [...document.querySelectorAll('.wizard button')].filter(b => b.textContent.trim() === 'Next').pop();
                      if (!b) return 'none'; b.click(); return 'ok';""") != "ok":
                    faults.append(f"wizard-{step} at {w}×{h}: cannot reach it")
                    break
                d.wait(1)
                faults += look(d, f"wizard-{step}", w, h, shots)
    return faults


if __name__ == "__main__":
    args = sys.argv[1:]
    shots = args[args.index("--shots") + 1] if "--shots" in args else None
    if shots:
        Path(shots).mkdir(parents=True, exist_ok=True)
    only = [a for a in args if not a.startswith("--") and a != shots] or None
    if not self_test():
        sys.exit(1)
    faults = check([(390, 844), (1100, 760), (1100, 700)], shots, only)
    for f in faults:
        print("FAULT", f)
    print(f"ui check: {len(faults)} faults")
    sys.exit(1 if faults else 0)
