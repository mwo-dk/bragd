// The Game last played in Play, per Family, so Analyse can take it up ("From the current
// game"). Only the record is kept: Rust replays it.

/** @type {Record<string, {variant: string, initial: string|null, entries: string[]}>} */
export const current = $state({});

/** A Game handed to Analyse from a replay, per Family: Analyse takes it up as it opens.
 * @type {Record<string, {variant: string, initial: string|null, entries: string[]}>} */
export const handed = $state({});
