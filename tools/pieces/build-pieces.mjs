// Builds the piece sprite from the artwork in assets/pieces/basic.
//
// Two things happen here. The pieces are collected into one SVG sprite of
// <symbol> elements, so the board can place them with <use> and they land as
// real elements in the DOM — which is what makes the board readable to a screen
// reader. And the two hard-coded colours are
// rewritten to CSS custom properties, so a board theme can repaint them; a
// piece referenced through <img> could not be themed at all, because custom
// properties do not cross that boundary.
//
// The artwork is Cburnett's, taken under the 3-clause BSD licence — one of four
// the author offered. Rewriting the colours makes the sprite a derivative work,
// which BSD permits without imposing its own terms on the result; what it does
// require is that the notice travel with the source, and an SVG shipped inside
// the app is source. Hence the notice in the generated header below.
//
// Its input is `assets/pieces/basic/`, which is itself generated — see
// `tools/pieces/normalise-upstream-pieces.mjs` for why the colours have to be
// normalised before the substitution here is safe. Licence and provenance:
// assets/pieces/LICENCE.md and docs/CREDITS.md.
//
// Symbol ids are `<sheet>-<side><ROLE>`, for example `chess-wK`; the scheme is
// in assets/pieces/README.md.
//
// Run: node tools/pieces/build-pieces.mjs

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const sprites = join(root, "gui", "src", "lib", "sprites");
const source = join(root, "assets", "pieces", "basic");
// Our own pieces, kept apart from the normalised upstream set.
const ours = join(root, "assets", "pieces", "shatranj");
// Capablanca's archbishop and chancellor, composed from two Cburnett pieces
// each — see tools/pieces/compose-capablanca-pieces.mjs.
const compounds = join(root, "assets", "pieces", "capablanca");

const ROLES = {
  pawn: "p",
  knight: "n",
  bishop: "b",
  rook: "r",
  queen: "q",
  king: "k",
};

// Both source colours mean something different depending on whose piece it is.
// On a White piece #FFF is the body and #000 the outline and detail; on a Black
// piece #000 is the body and #FFF the detail drawn on top of it. So the same
// literal maps to the fill on one side and to the line on the other.
const SUBSTITUTIONS = {
  w: [
    ["#FFF", "var(--piece-white-fill, #FFF)"],
    ["#000", "var(--piece-white-line, #000)"],
  ],
  b: [
    ["#000", "var(--piece-black-fill, #000)"],
    ["#FFF", "var(--piece-black-line, #FFF)"],
  ],
};

function innerSvg(text, file) {
  const match = text.match(/<svg\b[^>]*>([\s\S]*)<\/svg>/);
  if (!match) throw new Error(`${file}: no <svg> element`);
  return match[1];
}

function recolour(markup, color) {
  // Substitute in one pass. Doing it as two sequential replaces would rewrite
  // #000 to a var() containing "#000" and then rewrite that again.
  return markup.replace(/#FFF|#000/g, (literal) => {
    const found = SUBSTITUTIONS[color].find(([from]) => from === literal);
    if (!found) throw new Error(`unexpected colour ${literal}`);
    return found[1];
  });
}

// Shatranj's and chaturanga's pieces, drawn for this set.
//
// They live in their own directory because they are *ours*: `basic/` is
// generated from upstream and must stay a faithful normalisation of it, so
// original work does not belong there. They are drawn from Cburnett's pieces —
// the ferz on the queen's base, the alfil on the bishop's foot — which is
// allowed without inheriting a licence, because the set is taken under BSD
// rather than share-alike (assets/pieces/LICENCE.md).
//
// They were placeholders until now: a ferz shown as a queen and an alfil as a
// bishop. That was a lie a Player would believe, since neither moves anything
// like the piece it borrowed.
//
// **The gaja has its own picture now**, and sharing the alfil's was a mistake.
//
// The reasoning for sharing was that the two are the same animal — the war
// elephant — under two reconstructions of how it moved. That is true and it is
// not what matters on a board: they move quite differently, and a player
// looking at a position had nothing in the pieces to tell them which of the two
// games they were in. The alfil stays an abstract tusked dome, which is how the
// piece was carved for a thousand years; the gaja is an elephant, because that
// is what somebody expects when the game is called chaturanga.
const OURS = { F: "ferz", E: "alfil", G: "gaja" };

const present = new Set(readdirSync(source));
const symbols = [];

function symbol(color, letter, role, directory, family = "chess") {
  const file = `${role}-${color}.svg`;
  const markup = recolour(innerSvg(readFileSync(join(directory, file), "utf8"), file), color);
  return `\t<symbol id="${family}-${color}${letter.toUpperCase()}" viewBox="0 0 45 45">${markup.trimEnd()}\n\t</symbol>`;
}

for (const [role, letter] of Object.entries(ROLES)) {
  if (!present.has(`${role}-w.svg`)) throw new Error(`missing artwork: ${role}-w.svg`);
  for (const color of ["w", "b"]) symbols.push(symbol(color, letter, role, source));
}

for (const [letter, role] of Object.entries(OURS)) {
  for (const color of ["w", "b"]) symbols.push(symbol(color, letter, role, ours));
}

// Seirawan chess is a family of its own for the same reason Capablanca is: two
// pieces the chess set has no picture for, and a letter that collides. Its
// elephant is `S` here rather than `E`, matching its FEN letter, because `E` in
// the chess sprite is already shatranj's alfil. Ids stay unique across sheets
// as well, because all the sprites are inlined into one document where the
// first id wins silently.
const seirawanDir = join(root, "assets", "pieces", "seirawan");
const seirawan = [];
for (const [role, letter] of Object.entries(ROLES)) {
  for (const color of ["w", "b"])
    seirawan.push(symbol(color, letter, role, source, "seirawan"));
}
for (const color of ["w", "b"]) {
  seirawan.push(symbol(color, "H", "hawk", seirawanDir, "seirawan"));
}

// **Seirawan's elephant is chaturanga's elephant, and it is the same drawing.**
//
// It used to have its own: a rook's tower with a howdah and a raised trunk,
// on the reasoning that this elephant *moves* as a rook and the other two do
// not, so the picture should say rook. The reasoning is sound and the result
// was not. At board size the howdah reads as battlements and the trunk reads as
// a smudge on the left-hand side, so the piece read as **a rook something had
// gone wrong with** — which is worse than ugly, because a rook is a piece that
// is also on that board.
//
// The objection its comment raised against sharing — that the set has three
// animals called an elephant and they must be told apart — does not hold here.
// It matters between the alfil and the gaja, which share one sheet. Chaturanga
// and Seirawan share no board and no sheet: these ids are `seirawan-*` and
// those are `chess-*`, so there is not even a collision to have.
//
// What is left is what a player sees. One good elephant, drawn once.
for (const color of ["w", "b"]) {
  seirawan.push(symbol(color, "S", "gaja", ours, "seirawan"));
}

// Shogi's tiles are **not recoloured**, and that is the one exception in this
// file. Every other piece here is black-on-white or white-on-black and gets its
// two literals swapped for custom properties so a board theme can repaint it. A
// shogi piece is a wooden tile the same colour for both players — ownership is
// which way it points, not what colour it is — so there are no two literals to
// swap, and running the substitution over it would throw on the first unknown
// colour. Which it should: the check is what says this is deliberate.
const shogiDir = join(root, "assets", "pieces", "shogi");
const SHOGI = {
  p: "pawn",
  l: "lance",
  n: "knight",
  s: "silver",
  g: "gold",
  b: "bishop",
  r: "rook",
  k: "king",
  tp: "tokin",
  tl: "promoted-lance",
  tn: "promoted-knight",
  ts: "promoted-silver",
  h: "horse",
  d: "dragon",
};
const shogi = [];
for (const [letter, role] of Object.entries(SHOGI)) {
  for (const color of ["w", "b"]) {
    const file = `${role}-${color}.svg`;
    const markup = innerSvg(readFileSync(join(shogiDir, file), "utf8"), file);
    shogi.push(
      `\t<symbol id="shogi-${color}${letter.toUpperCase()}" viewBox="0 0 45 45">${markup.trimEnd()}\n\t</symbol>`,
    );
  }
}

// Capablanca chess has its own rules crate, because eighty squares do not fit
// in a u64, so it gets a sprite of its own rather than
// borrowing the chess one. Its six ordinary pieces are the same artwork; ids
// are namespaced by sheet, and two sheets may not share an id, because all
// the sprites are inlined into one document and the first id wins silently.
const capablanca = [];
for (const [role, letter] of Object.entries(ROLES)) {
  for (const color of ["w", "b"])
    capablanca.push(symbol(color, letter, role, source, "capablanca"));
}
for (const [letter, role] of Object.entries({ A: "archbishop", C: "chancellor" })) {
  for (const color of ["w", "b"])
    capablanca.push(symbol(color, letter, role, compounds, "capablanca"));
}

// One sheet template per notice: the provenance is not the same for every
// family, and a header that always said Cburnett lied about the shogi kanji
// for a while. A licence header is the one string a copy-paste can turn into
// a false claim, so the notice is a parameter.
const sheet = (notice, syms) => `<?xml version="1.0" encoding="UTF-8"?>
<!--
	GENERATED by tools/pieces/build-pieces.mjs — do not edit by hand.

${notice}
-->
<svg xmlns="http://www.w3.org/2000/svg" style="display: none" aria-hidden="true">
${syms.join("\n")}
</svg>
`;

const CBURNETT = `	Artwork by Cburnett, from Wikimedia Commons, beginning at
	https://commons.wikimedia.org/wiki/File:Chess_klt45.svg
	Multi-licensed by its author under GFDL 1.2+, CC BY-SA 3.0, GPL v2+ and the
	3-clause BSD licence. Taken here under the 3-clause BSD licence.

	Modified: colours replaced with CSS custom properties so a board theme can
	repaint the pieces, and six-digit colour codes rewritten as three-digit ones
	so that substitution is safe. No path data was changed.

	Copyright (c) Cburnett. Redistribution and use in source and binary forms,
	with or without modification, are permitted provided that the following
	conditions are met: redistributions of source code must retain the above
	copyright notice, this list of conditions and the following disclaimer;
	redistributions in binary form must reproduce them in the documentation
	and/or other materials provided with the distribution; and neither the name
	of the author nor the names of contributors may be used to endorse or
	promote products derived from this software without specific prior written
	permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
	CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT
	NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
	PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
	CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
	EXEMPLARY, OR CONSEQUENTIAL DAMAGES HOWEVER CAUSED AND ON ANY THEORY OF
	LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT, ARISING IN ANY
	WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
	SUCH DAMAGE.`;

const NOTO = `	The kanji are glyph outlines from Noto Sans CJK (by Google), taken as
	bezier paths by tools/pieces/build-shogi-pieces.py under the SIL Open Font License
	1.1 — converting text to outlines for a document is a use the OFL permits,
	and this file redistributes those outlines, not the font. The wedge tile
	they sit on is drawn for this set and carries no obligation of its own.
	assets/pieces/LICENCE.md holds the whole account.`;

const sprite = sheet(CBURNETT, symbols);

writeFileSync(join(sprites, "chess-pieces.svg"), sprite);

// The Capablanca sheet: the same artwork, so the same notice.
writeFileSync(
  join(sprites, "capablanca-pieces.svg"),
  sheet(CBURNETT, capablanca),
);

// The shogi sheet carries its own notice: the kanji are Noto Sans CJK
// outlines, not Cburnett's artwork.
writeFileSync(join(sprites, "shogi-pieces.svg"), sheet(NOTO, shogi));

// The Seirawan sheet: the same twelve chess pieces plus the hawk and the
// elephant, under its own family so no id can collide with the chess set's.
// Cburnett's notice, because Cburnett's pieces are in it.
writeFileSync(
  join(sprites, "seirawan-pieces.svg"),
  sheet(CBURNETT, seirawan),
);

console.log(
  `wrote ${symbols.length} chess pieces, ` +
    `${capablanca.length} Capablanca, ${seirawan.length} Seirawan and ` +
    `${shogi.length} shogi pieces`,
);
