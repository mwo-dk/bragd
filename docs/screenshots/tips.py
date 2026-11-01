"""The pictures of the install wizard's step 2 (Your games), a Tip, and Settings → Tips, each at
390 and 1100 px, and Settings at Overview and Game families at 1100 px.

    DEMO_HOME=<dir> uv run --with websocket-client python tips.py [out-dir]
"""

import sys
from pathlib import Path

from cap import HERE, SEEN, Desktop, widths
from pages import click

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE


def games(d):
    d.wait(2)
    click(d, "Next", ".wizard footer")


# A start with nothing used and no Tip yet: the first Tip is due at once.
TIP = dict(SEEN, tip_shown=0)
QUIET = SEEN

widths("wizard-games", games, out=OUT, state={})
widths("tip", lambda d: d.wait(3), out=OUT, state=TIP)
def settings_tips(d):
    if d.size[0] < 760:  # the sidebar starts folded on a narrow window
        click(d, "»", "nav")
    click(d, "Settings", "nav")
    click(d, "Tips", "main")


widths("settings-tips", settings_tips, out=OUT, state=QUIET)

with Desktop(size=(1100, 760), state=QUIET) as d:
    click(d, "Settings", "nav")
    d.shot(OUT / "settings-overview-1100.png")
    click(d, "Game families", "main")
    d.shot(OUT / "settings-families-1100.png")
