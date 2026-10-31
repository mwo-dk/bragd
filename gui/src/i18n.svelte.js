// Texts in the user's language. The catalogue comes from bragd-core (crates/bragd-core/src/
// i18n.rs and locales/), already merged with British English, the reference. Reading `i18n`
// inside t() makes every text reactive: switching the language redraws all of them.

export const i18n = $state({ lang: "en-GB", strings: /** @type {Record<string, any>} */ ({}), rtl: false });

let rules = new Intl.PluralRules("en-GB");

/** Use a new language (from a snapshot). */
export function setLanguage(/** @type {{language: string, strings: Record<string, any>, rtl: boolean}} */ snap) {
  i18n.lang = snap.language;
  i18n.strings = snap.strings;
  i18n.rtl = snap.rtl;
  rules = new Intl.PluralRules(snap.language);
  document.documentElement.lang = snap.language;
  document.documentElement.dir = snap.rtl ? "rtl" : "ltr";
}

/** @param {string} s @param {Record<string, any>} params */
const fill = (s, params) => s.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));

/** The text for `key` with `{name}` placeholders filled. A missing key shows as itself. */
export function t(/** @type {string} */ key, /** @type {Record<string, any>} */ params = {}) {
  const v = i18n.strings[key];
  if (typeof v === "string") return fill(v, params);
  if (v && typeof v === "object") return fill(v.other ?? key, params);
  return key;
}

/** The text for `key` and count `n`, in the plural form the language uses; `{n}` is the count. */
export function tn(/** @type {string} */ key, /** @type {number} */ n, /** @type {Record<string, any>} */ params = {}) {
  const v = i18n.strings[key];
  const all = { n: num(n), ...params };
  if (v && typeof v === "object") return fill(v[rules.select(n)] ?? v.other ?? key, all);
  return typeof v === "string" ? fill(v, all) : key;
}

/** A number formatted for the language. */
export const num = (/** @type {number} */ n) => Number(n).toLocaleString(i18n.lang);
