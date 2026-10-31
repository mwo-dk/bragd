// Draws Capablanca's two compound pieces by composing Cburnett's.
//
// An archbishop is a bishop and a knight in one piece; a chancellor is a rook
// and a knight. That is literally how they move — the union of two pieces that
// already exist — so drawing them as the union of two pictures that already
// exist is the honest depiction rather than a shortcut, and it is what most
// published Capablanca sets do.
//
// The knight is laid over the lower body of the other piece at a little under
// half scale. It is drawn *after*, so its white fill covers what it overlaps
// and the silhouette stays readable at board size — which is the only size that
// matters, and the reason this was checked by rendering it rather than by
// reasoning about the numbers.
//
// The set is taken under the 3-clause BSD licence (assets/pieces/LICENCE.md), which permits
// derivative works without imposing terms on them. The notice travels with the
// sprite, as it must.
//
// shortcut: this draws the earlier design (a small knight inside the piece);
// the committed archbishop and chancellor were revised by hand into a base graft
// (their headers say why). Running it overwrites them with the old design.
// Update it to the graft before running it again.
//
// Run: node tools/pieces/compose-capablanca-pieces.mjs

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const source = join(root, "assets", "pieces", "basic");
const destination = join(root, "assets", "pieces", "capablanca");

/** Everything inside the `<svg>` element. */
function inner(file) {
  const text = readFileSync(join(source, file), "utf8");
  const match = text.match(/<svg\b[^>]*>([\s\S]*)<\/svg>/);
  if (!match) throw new Error(`${file}: no <svg> element`);
  return match[1].trim();
}

// Where the small knight sits. The base pieces are 45x45 with their feet at
// y=39; the knight's own drawing spans roughly x 6-38, y 7-39, so at 0.46 it is
// about 15 units across and is placed low and slightly right of centre — over
// the body of the piece beneath, not over its head, which is what keeps both
// silhouettes legible.
const KNIGHT = { scale: 0.46, x: 14.5, y: 17.5 };

const PIECES = {
  archbishop: "bishop",
  chancellor: "rook",
};

mkdirSync(destination, { recursive: true });

for (const [name, base] of Object.entries(PIECES)) {
  for (const color of ["w", "b"]) {
    const body = inner(`${base}-${color}.svg`);
    // **The overlaid knight always comes from the white artwork**, on both
    // colours, and that is not a copy-paste slip.
    //
    // The set draws a White piece as a light body with dark lines, and a Black
    // piece as a dark body with its *detail picked out in the light colour on
    // top*. A black knight laid on a black rook obeys the first rule and breaks
    // the second: two dark shapes overlapping, with only a hairline between
    // them, which is what the first attempt looked like and why it was wrong.
    //
    // Taking the white knight puts `#FFF` where the fill goes, and the colour
    // substitution downstream maps that to the *line* colour on a black piece —
    // so it comes out as a light horse outlined in dark, sitting on the dark
    // body. Detail on top, which is the set's own rule.
    const knight = inner("knight-w.svg");
    const markup = `<?xml version="1.0" encoding="UTF-8"?>
<!--
	${name}-${color}

	COMPOSED by tools/pieces/compose-capablanca-pieces.mjs from Cburnett's ${base} and
	knight — do not edit by hand.

	A ${name} moves as a ${base} and a knight at once, so it is drawn as both.

	Original artwork by Cburnett, from Wikimedia Commons, beginning at
	https://commons.wikimedia.org/wiki/File:Chess_klt45.svg
	Multi-licensed by its author under GFDL 1.2+, CC BY-SA 3.0, GPL v2+ and the
	3-clause BSD licence. Taken here under the 3-clause BSD licence, which permits
	this modification without imposing its terms on the result. Full notice:
	assets/pieces/LICENCE.md and docs/CREDITS.md.
-->
<svg xmlns="http://www.w3.org/2000/svg" width="45" height="45" viewBox="0 0 45 45">
  ${body}
  <g transform="translate(${KNIGHT.x} ${KNIGHT.y}) scale(${KNIGHT.scale})">
    ${knight}
  </g>
</svg>
`;
    writeFileSync(join(destination, `${name}-${color}.svg`), markup);
  }
}

console.log(`composed ${Object.keys(PIECES).length * 2} pieces`);
