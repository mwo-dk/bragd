//! Translations. Each language is a flat JSON catalogue in `locales/` (`"key": "text"`),
//! compiled into the binary. British English (`en-GB`) is the reference: every key exists
//! there, and any key a language lacks falls back to it. The other English variants hold only
//! the words that differ; so do the German, French, Italian, Dutch and Spanish variants, which
//! fall back to their base language first (`chain`). Swiss German is German with every ß
//! written ss, made when the catalogues load, plus its own file. Texts take `{name}`
//! placeholders; a text that depends on a count is an object with one entry per plural category
//! of its language (`one`, `few`, `many`, `zero`, `other`).

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// A language Bragd speaks.
#[derive(Debug, serde::Serialize)]
pub struct Language {
    pub code: &'static str,
    /// Its name in itself.
    pub name: &'static str,
    /// A `flag-icons` name, or `derafsh`: Persian's banner.
    pub flag: &'static str,
    /// The region it is listed under: a `language.group.*` key.
    pub group: &'static str,
    /// Written right to left: the sidebar goes on the right.
    pub rtl: bool,
}

const fn lang(
    code: &'static str,
    name: &'static str,
    flag: &'static str,
    group: &'static str,
) -> Language {
    let rtl = matches!(code.as_bytes(), b"he" | b"fa");
    Language {
        code,
        name,
        flag,
        group,
        rtl,
    }
}

/// Every language, by region and by its own name within one: the list Settings shows.
pub const LANGUAGES: &[Language] = &[
    lang("da", "Dansk", "dk", "language.group.nordic"),
    lang("et", "Eesti", "ee", "language.group.nordic"),
    lang("lv", "Latviešu", "lv", "language.group.nordic"),
    lang("lt", "Lietuvių", "lt", "language.group.nordic"),
    lang("nb", "Norsk (bokmål)", "no", "language.group.nordic"),
    lang("nn", "Norsk (nynorsk)", "no", "language.group.nordic"),
    lang("fi", "Suomi", "fi", "language.group.nordic"),
    lang("sv", "Svenska", "se", "language.group.nordic"),
    lang("ca", "Català", "es-ct", "language.group.western"),
    lang("de", "Deutsch", "de", "language.group.western"),
    lang(
        "de-AT",
        "Deutsch (Österreich)",
        "at",
        "language.group.western",
    ),
    lang("de-CH", "Deutsch (Schweiz)", "ch", "language.group.western"),
    lang(
        "en-GB",
        "English (United Kingdom)",
        "gb",
        "language.group.western",
    ),
    lang("eu", "Euskara", "es-pv", "language.group.western"),
    lang("fr", "Français", "fr", "language.group.western"),
    lang(
        "fr-BE",
        "Français (Belgique)",
        "be",
        "language.group.western",
    ),
    lang("fr-CH", "Français (Suisse)", "ch", "language.group.western"),
    lang("it", "Italiano", "it", "language.group.western"),
    lang(
        "it-CH",
        "Italiano (Svizzera)",
        "ch",
        "language.group.western",
    ),
    lang("nl", "Nederlands", "nl", "language.group.western"),
    lang(
        "nl-BE",
        "Nederlands (België)",
        "be",
        "language.group.western",
    ),
    lang("cs", "Čeština", "cz", "language.group.central"),
    lang("pl", "Polski", "pl", "language.group.central"),
    lang("uk", "Українська", "ua", "language.group.central"),
    lang("el", "Ελληνικά", "gr", "language.group.mediterranean"),
    lang("he", "עברית", "il", "language.group.middle_east"),
    lang("fa", "فارسی", "derafsh", "language.group.middle_east"),
    lang("hy", "Հայերեն", "am", "language.group.caucasus"),
    lang("ka", "ქართული", "ge", "language.group.caucasus"),
    lang("en-CA", "English (Canada)", "ca", "language.group.americas"),
    lang(
        "es-AR",
        "Español (Argentina)",
        "ar",
        "language.group.americas",
    ),
    lang(
        "es-BO",
        "Español (Bolivia)",
        "bo",
        "language.group.americas",
    ),
    lang("es-CL", "Español (Chile)", "cl", "language.group.americas"),
    lang(
        "es-CO",
        "Español (Colombia)",
        "co",
        "language.group.americas",
    ),
    lang(
        "es-CR",
        "Español (Costa Rica)",
        "cr",
        "language.group.americas",
    ),
    lang("es-CU", "Español (Cuba)", "cu", "language.group.americas"),
    lang(
        "es-EC",
        "Español (Ecuador)",
        "ec",
        "language.group.americas",
    ),
    lang(
        "es-SV",
        "Español (El Salvador)",
        "sv",
        "language.group.americas",
    ),
    lang(
        "es-GT",
        "Español (Guatemala)",
        "gt",
        "language.group.americas",
    ),
    lang(
        "es-HN",
        "Español (Honduras)",
        "hn",
        "language.group.americas",
    ),
    lang("es-MX", "Español (México)", "mx", "language.group.americas"),
    lang(
        "es-NI",
        "Español (Nicaragua)",
        "ni",
        "language.group.americas",
    ),
    lang("es-PA", "Español (Panamá)", "pa", "language.group.americas"),
    lang("es-PE", "Español (Perú)", "pe", "language.group.americas"),
    lang(
        "es-PR",
        "Español (Puerto Rico)",
        "pr",
        "language.group.americas",
    ),
    lang(
        "es-DO",
        "Español (República Dominicana)",
        "do",
        "language.group.americas",
    ),
    lang(
        "es-UY",
        "Español (Uruguay)",
        "uy",
        "language.group.americas",
    ),
    lang(
        "es-VE",
        "Español (Venezuela)",
        "ve",
        "language.group.americas",
    ),
    lang(
        "fr-CA",
        "Français (Canada)",
        "ca",
        "language.group.americas",
    ),
    lang("en-AU", "English (Australia)", "au", "language.group.asia"),
    lang(
        "en-NZ",
        "English (New Zealand)",
        "nz",
        "language.group.asia",
    ),
    lang("zh-TW", "中文（台灣）", "tw", "language.group.asia"),
    lang("zh-SG", "中文（新加坡）", "sg", "language.group.asia"),
    lang("ja", "日本語", "jp", "language.group.asia"),
    lang("ko", "한국어", "kr", "language.group.asia"),
];

/// The language `code` (one of `LANGUAGES`).
pub fn find(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.code == code)
}

/// The key of the text `shown` in the language `code`: the one that says exactly that (case
/// and spaces aside), else the shortest that holds it. None for an empty or unknown text.
pub fn key_of(code: &str, shown: &str) -> Option<String> {
    let want = shown.trim().to_lowercase();
    if want.is_empty() {
        return None;
    }
    let all = catalogue(code);
    let text = |v: &serde_json::Value| match v {
        serde_json::Value::String(s) => s.to_lowercase(),
        v => v
            .get("other")
            .and_then(|o| o.as_str())
            .unwrap_or("")
            .to_lowercase(),
    };
    let mut keys: Vec<(&String, String)> = all.iter().map(|(k, v)| (k, text(v))).collect();
    keys.sort();
    keys.iter()
        .find(|(_, t)| t.trim() == want)
        .or_else(|| {
            keys.iter()
                .filter(|(_, t)| t.contains(&want))
                .min_by_key(|(_, t)| t.len())
        })
        .map(|(k, _)| k.to_string())
}

/// *Suggest a better translation*: a new GitHub issue in the browser, its title and body
/// naming only the language `code`, this version and, when the user typed the text to mend
/// (`shown`), that text and its key. Bragd sends nothing itself.
pub fn suggest_url(code: &str, shown: &str) -> String {
    let key = key_of(code, shown);
    let title = format!("Translation ({code}): {}", key.as_deref().unwrap_or(""));
    let now = match &key {
        Some(k) => format!("Key: {k}\nIt says now: {}\n", tr_in(code, k)),
        None => String::new(),
    };
    let body = format!(
        "Language: {code}\nBragd {}\n{now}\nWhere the text shows, and what it says now:\n\n\nWhat it should say:\n",
        env!("CARGO_PKG_VERSION")
    );
    // Everything but the unreserved characters percent-encoded, byte by byte.
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "https://github.com/mwo-dk/bragd/issues/new?title={}&body={}",
        enc(&title),
        enc(&body)
    )
}

/// Languages written right to left: the desktop app mirrors its layout for them.
pub fn is_rtl(code: &str) -> bool {
    find(code).is_some_and(|l| l.rtl)
}

/// `s` with its digits in the current language's own: Persian writes ۰–۹. Only for numbers
/// shown as numbers (counts, sizes, dates), never paths, keys or config values.
pub fn digits(s: String) -> String {
    digits_in(language(), s)
}

fn digits_in(lang: &str, s: String) -> String {
    if lang != "fa" {
        return s;
    }
    s.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                char::from_u32('۰' as u32 + (c as u32 - '0' as u32)).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// `n` bytes as shown, in powers of 1000 as disks are sold, with the current language's
/// decimal mark: `1.5 GB`, `1,5 GB`.
pub fn bytes(n: u64) -> String {
    bytes_in(language(), n)
}

fn bytes_in(lang: &str, n: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB"];
    let (mut v, mut u) = (n as f64, 0);
    while v >= 1000.0 && u + 1 < units.len() {
        v /= 1000.0;
        u += 1;
    }
    if u == 0 {
        return digits_in(lang, format!("{n} B"));
    }
    // shortcut: the decimal mark by language, as CLDR has it for these; take ICU data if
    // grouping or other marks are ever needed.
    let point = lang.starts_with("en")
        || lang.starts_with("zh")
        || matches!(lang, "he" | "ja" | "ko" | "de-CH" | "it-CH" | "es-MX" | "es-PR" | "es-DO" | "es-GT" | "es-HN" | "es-NI" | "es-PA" | "es-SV");
    let mark = if lang == "fa" { "\u{066B}" } else if point { "." } else { "," };
    digits_in(lang, format!("{v:.1} {}", units[u]).replacen('.', mark, 1))
}

pub const REFERENCE: &str = "en-GB";

fn source(code: &str) -> &'static str {
    match code {
        "da" => include_str!("../locales/da.json"),
        "et" => include_str!("../locales/et.json"),
        "lv" => include_str!("../locales/lv.json"),
        "lt" => include_str!("../locales/lt.json"),
        "nb" => include_str!("../locales/nb.json"),
        "nn" => include_str!("../locales/nn.json"),
        "fi" => include_str!("../locales/fi.json"),
        "sv" => include_str!("../locales/sv.json"),
        "ca" => include_str!("../locales/ca.json"),
        "de" => include_str!("../locales/de.json"),
        "de-AT" => include_str!("../locales/de-AT.json"),
        "de-CH" => include_str!("../locales/de-CH.json"),
        "en-GB" => include_str!("../locales/en-GB.json"),
        "eu" => include_str!("../locales/eu.json"),
        "fr" => include_str!("../locales/fr.json"),
        "fr-BE" => include_str!("../locales/fr-BE.json"),
        "fr-CH" => include_str!("../locales/fr-CH.json"),
        "it" => include_str!("../locales/it.json"),
        "it-CH" => include_str!("../locales/it-CH.json"),
        "nl" => include_str!("../locales/nl.json"),
        "nl-BE" => include_str!("../locales/nl-BE.json"),
        "cs" => include_str!("../locales/cs.json"),
        "pl" => include_str!("../locales/pl.json"),
        "uk" => include_str!("../locales/uk.json"),
        "el" => include_str!("../locales/el.json"),
        "he" => include_str!("../locales/he.json"),
        "fa" => include_str!("../locales/fa.json"),
        "hy" => include_str!("../locales/hy.json"),
        "ka" => include_str!("../locales/ka.json"),
        "en-CA" => include_str!("../locales/en-CA.json"),
        "es-AR" => include_str!("../locales/es-AR.json"),
        "es-BO" => include_str!("../locales/es-BO.json"),
        "es-CL" => include_str!("../locales/es-CL.json"),
        "es-CO" => include_str!("../locales/es-CO.json"),
        "es-CR" => include_str!("../locales/es-CR.json"),
        "es-CU" => include_str!("../locales/es-CU.json"),
        "es-EC" => include_str!("../locales/es-EC.json"),
        "es-SV" => include_str!("../locales/es-SV.json"),
        "es-GT" => include_str!("../locales/es-GT.json"),
        "es-HN" => include_str!("../locales/es-HN.json"),
        "es-MX" => include_str!("../locales/es-MX.json"),
        "es-NI" => include_str!("../locales/es-NI.json"),
        "es-PA" => include_str!("../locales/es-PA.json"),
        "es-PE" => include_str!("../locales/es-PE.json"),
        "es-PR" => include_str!("../locales/es-PR.json"),
        "es-DO" => include_str!("../locales/es-DO.json"),
        "es-UY" => include_str!("../locales/es-UY.json"),
        "es-VE" => include_str!("../locales/es-VE.json"),
        "fr-CA" => include_str!("../locales/fr-CA.json"),
        "en-AU" => include_str!("../locales/en-AU.json"),
        "en-NZ" => include_str!("../locales/en-NZ.json"),
        "zh-TW" => include_str!("../locales/zh-TW.json"),
        "zh-SG" => include_str!("../locales/zh-SG.json"),
        "ja" => include_str!("../locales/ja.json"),
        "ko" => include_str!("../locales/ko.json"),
        _ => "{}",
    }
}

/// The language to use for a BCP 47 tag, the nearest one Bragd has: the language itself in
/// any region, else a close relative, else British English.
pub fn nearest(tag: &str) -> &'static str {
    let tag = tag.replace('_', "-");
    let lower = tag.to_ascii_lowercase();
    let lang = lower.split(['-', '.', '@']).next().unwrap_or("");
    let region = lower.split(['-', '.', '@']).nth(1).unwrap_or("");
    if lang == "en" {
        return match region {
            "au" => "en-AU",
            "ca" => "en-CA",
            "nz" => "en-NZ",
            // American English gets Canadian, the nearest in spelling; every other English
            // gets British.
            "us" | "" => "en-CA",
            _ => "en-GB",
        };
    }
    match lang {
        "da" => "da",
        // Norwegian without a written standard named (`no`, `no_NO`) is Bokmål, as most write it.
        "nb" | "no" => "nb",
        "nn" => "nn",
        "sv" => "sv",
        "fi" => "fi",
        "et" => "et",
        "lv" => "lv",
        "lt" => "lt",
        // Liechtenstein writes Swiss Standard German, as does Swiss German (gsw).
        // South Tyrol (de-IT) writes Austrian German.
        "de" => match region {
            "at" | "it" => "de-AT",
            "ch" | "li" => "de-CH",
            _ => "de",
        },
        "gsw" => "de-CH",
        "lb" => "de",
        "fr" => match region {
            "ca" => "fr-CA",
            "be" => "fr-BE",
            "ch" | "li" => "fr-CH",
            _ => "fr",
        },
        "it" => match region {
            "ch" => "it-CH",
            _ => "it",
        },
        // Flemish is Dutch as Belgium writes it; West Flemish (vls) readers read it.
        "nl" if region == "be" => "nl-BE",
        "vls" => "nl-BE",
        "nl" | "fy" | "af" => "nl",
        // Latin America writes tú, except the vos of Argentina, Uruguay and Paraguay: its own
        // entry where it has one, else Chilean Spanish (vos: Argentinian). Spain, Galician and Aragonese stay on
        // Argentinian Spanish.
        "es" => match region {
            "cl" | "419" | "us" => "es-CL",
            "bo" => "es-BO",
            "ec" => "es-EC",
            "co" => "es-CO",
            "cr" => "es-CR",
            "cu" => "es-CU",
            "sv" => "es-SV",
            "gt" => "es-GT",
            "hn" => "es-HN",
            "mx" => "es-MX",
            "ni" => "es-NI",
            "pa" => "es-PA",
            "pe" => "es-PE",
            "pr" => "es-PR",
            "do" => "es-DO",
            "ve" => "es-VE",
            "uy" => "es-UY",
            _ => "es-AR",
        },
        "gl" | "an" => "es-AR",
        "ca" | "oc" => "ca",
        "eu" => "eu",
        "he" | "iw" => "he",
        "pl" => "pl",
        "cs" | "sk" => "cs", // Slovak readers read Czech
        "uk" => "uk",
        "el" => "el",
        "ja" => "ja",
        "ko" => "ko",
        // Chinese by its script.
        "zh" => {
            let parts: Vec<&str> = lower.split(['-', '.', '@']).collect();
            if parts.contains(&"hant")
                || (!parts.contains(&"hans")
                    && parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")))
            {
                "zh-TW"
            } else {
                "zh-SG"
            }
        }
        // Dari (Afghan Persian) has its own code; Tajik is Persian in Cyrillic, so not here.
        "fa" | "prs" => "fa",
        "ka" => "ka",
        "hy" => "hy",
        _ => REFERENCE,
    }
}

/// `"auto"` (or empty): the system's languages, first one Bragd has; else the given code.
pub fn resolve(pref: &str) -> &'static str {
    if !pref.is_empty() && pref != "auto" {
        return nearest(pref);
    }
    pick(&sys_locale::get_locales().collect::<Vec<_>>())
}

/// The first of the system's languages Bragd has (any English counts: British English is
/// also the fallback for the ones it lacks), else the nearest to the first one.
fn pick(system: &[String]) -> &'static str {
    system
        .iter()
        .find(|l| nearest(l) != REFERENCE || l.to_ascii_lowercase().starts_with("en"))
        .or(system.first())
        .map_or(REFERENCE, |l| nearest(l))
}

type Map = HashMap<String, serde_json::Value>;

/// Swiss Standard German from German: ss for ß, and «guillemets» for „quotes“ (keys are
/// ASCII, so only texts change).
fn swiss(de: &str) -> String {
    de.replace('ß', "ss")
        .replace('ẞ', "SS")
        .replace('„', "«")
        .replace('“', "»")
}

/// A language's catalogue (de-CH: German made Swiss, under its own texts), parsed the first
/// time it is asked for: a start reads the two or three files its language falls back
/// through, not all of them.
fn cat(code: &str) -> Option<&'static Map> {
    const N: usize = LANGUAGES.len();
    static C: [OnceLock<Map>; N] = [const { OnceLock::new() }; N];
    let i = LANGUAGES.iter().position(|l| l.code == code)?;
    Some(C[i].get_or_init(|| {
        let parse = |s: &str| -> Map { serde_json::from_str(s).unwrap_or_default() };
        let own = parse(source(code));
        if code != "de-CH" {
            return own;
        }
        let mut all = parse(&swiss(source("de")));
        all.extend(own);
        all
    }))
}

/// The language a variant takes what it lacks from: its base language (`de-AT` → `de`); for
/// Spanish, Argentinian (the first Spanish translated), and the Spanish that writes tú takes Chilean first.
fn parent(code: &str) -> Option<&'static str> {
    match code {
        "es-AR" => None,
        // Uruguay writes vos, as Argentina does.
        "es-CL" | "es-UY" => Some("es-AR"),
        _ if code.starts_with("es-") => Some("es-CL"),
        _ => find(code.split('-').next().unwrap_or(code))
            .filter(|l| l.code != code)
            .map(|l| l.code),
    }
}

/// The fallback chain for a language: itself, its parents (`parent`), then British English.
pub(crate) fn chain(code: &str) -> Vec<&str> {
    let mut v = vec![code];
    let mut c = code;
    while let Some(p) = parent(c) {
        v.push(p);
        c = p;
    }
    v.push(REFERENCE);
    v
}

/// The whole catalogue for `code`, with fallbacks filled in, for the desktop app.
pub fn catalogue(code: &str) -> Map {
    let mut out = Map::new();
    for c in chain(code).into_iter().rev() {
        if let Some(own) = cat(c) {
            out.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
    out
}

static CURRENT: RwLock<&'static str> = RwLock::new(REFERENCE);

pub fn set_language(code: &str) {
    if let Ok(mut c) = CURRENT.write() {
        *c = nearest(code);
    }
}

pub fn language() -> &'static str {
    CURRENT.read().map(|c| *c).unwrap_or(REFERENCE)
}

fn lookup(code: &str, key: &str) -> Option<&'static serde_json::Value> {
    chain(code).into_iter().find_map(|c| cat(c)?.get(key))
}

fn fill(text: &str, args: &[(&str, &str)]) -> String {
    args.iter().fold(text.to_string(), |s, (k, v)| {
        s.replace(&format!("{{{k}}}"), v)
    })
}

/// The text for `key` in the current language, `{name}` placeholders filled from `args`. A
/// missing key shows as the key itself, so it is easy to spot.
pub fn tr(key: &str, args: &[(&str, &str)]) -> String {
    match lookup(language(), key) {
        Some(serde_json::Value::String(s)) => fill(s, args),
        Some(serde_json::Value::Object(o)) => o
            .get("other")
            .and_then(|v| v.as_str())
            .map_or_else(|| key.to_string(), |s| fill(s, args)),
        _ => key.to_string(),
    }
}

/// The text for `key` in the language `code`, whatever the current one is (the docs' English).
pub fn tr_in(code: &str, key: &str) -> String {
    lookup(code, key)
        .and_then(|v| v.as_str())
        .map_or_else(|| key.to_string(), str::to_string)
}

/// The text for `key` and count `n`, in the plural form the current language uses for it.
/// `{n}` is available as a placeholder.
pub fn trn(key: &str, n: u64, args: &[(&str, &str)]) -> String {
    let lang = language();
    let count = digits(n.to_string());
    let mut all: Vec<(&str, &str)> = vec![("n", count.as_str())];
    all.extend_from_slice(args);
    match lookup(lang, key) {
        Some(serde_json::Value::Object(o)) => {
            let form = o
                .get(plural(lang, n))
                .or_else(|| o.get("other"))
                .and_then(|v| v.as_str())
                .unwrap_or(key);
            fill(form, &all)
        }
        Some(serde_json::Value::String(s)) => fill(s, &all),
        _ => key.to_string(),
    }
}

/// The CLDR plural category of a whole number `n` in `lang` (the same as the browser's
/// `Intl.PluralRules`, which the desktop app uses).
pub fn plural(lang: &str, n: u64) -> &'static str {
    let (m10, m100) = (n % 10, n % 100);
    match lang.split('-').next().unwrap_or(lang) {
        "lv" => {
            if m10 == 0 || (11..=19).contains(&m100) {
                "zero"
            } else if m10 == 1 && m100 != 11 {
                "one"
            } else {
                "other"
            }
        }
        "lt" => {
            if m10 == 1 && !(11..=19).contains(&m100) {
                "one"
            } else if (2..=9).contains(&m10) && !(11..=19).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        // French: 0 and 1 are singular; a million and up is "many".
        "fr" => {
            if n < 2 {
                "one"
            } else if n != 0 && n.is_multiple_of(1_000_000) {
                "many"
            } else {
                "other"
            }
        }
        // Polish, Ukrainian: 2–4 (not 12–14) is "few", other whole numbers "many"; Ukrainian
        // also takes 21, 31, … as "one". "other" is for fractions.
        "pl" | "uk" => {
            if n == 1 || (lang.starts_with("uk") && m10 == 1 && m100 != 11) {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        // Czech: 2–4 is "few"; "many" is only for fractions.
        "cs" => match n {
            1 => "one",
            2..=4 => "few",
            _ => "other",
        },
        "ja" | "ko" | "zh" => "other",
        // Persian and Armenian: 0 and 1 are singular (CLDR: i = 0 or n = 1).
        "fa" | "hy" => {
            if n < 2 {
                "one"
            } else {
                "other"
            }
        }
        "he" => match n {
            1 => "one",
            2 => "two",
            _ => "other",
        },
        // Spanish, Italian and Catalan also use "many" for round millions.
        "es" | "it" | "ca" if n != 0 && n.is_multiple_of(1_000_000) => "many",
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// `tr` as a macro, for brevity: `t!("update.available", "version" => v)`.
#[macro_export]
macro_rules! t {
    ($key:expr) => { $crate::i18n::tr($key, &[]) };
    ($key:expr, $($k:literal => $v:expr),+ $(,)?) => { $crate::i18n::tr($key, &[$(($k, &$v.to_string())),+]) };
}

/// `trn` as a macro: `tn!("items", 3)`, `tn!("files.found", n, "dir" => d)`.
#[macro_export]
macro_rules! tn {
    ($key:expr, $n:expr) => { $crate::i18n::trn($key, $n as u64, &[]) };
    ($key:expr, $n:expr, $($k:literal => $v:expr),+ $(,)?) => { $crate::i18n::trn($key, $n as u64, &[$(($k, &$v.to_string())),+]) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i18n_suggest_url_names_only_the_language_version_and_text() {
        let url = suggest_url("de-CH", "");
        assert!(
            url.starts_with("https://github.com/mwo-dk/bragd/issues/new?title=Translation%20%28de-CH%29%3A%20&body=Language%3A%20de-CH%0ABragd%20"),
            "{url}"
        );
        assert_eq!(
            key_of(REFERENCE, " settings "),
            Some("settings.title".into())
        );
        assert_eq!(
            key_of(REFERENCE, "fold the sidebar"),
            Some("sidebar.collapse".into())
        );
        assert_eq!(key_of(REFERENCE, "zzzz no such text"), None);
        let url = suggest_url(REFERENCE, "Settings");
        assert!(
            url.contains("title=Translation%20%28en-GB%29%3A%20settings.title&"),
            "{url}"
        );
        assert!(
            url.contains("Key%3A%20settings.title%0AIt%20says%20now%3A%20Settings"),
            "{url}"
        );
    }

    #[test]
    fn i18n_every_locale_file_is_a_language() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_suffix(".json")
                    .map(String::from)
            })
            .collect();
        files.sort();
        let mut codes: Vec<String> = LANGUAGES.iter().map(|l| l.code.to_string()).collect();
        codes.sort();
        assert_eq!(files, codes);
        assert_eq!(codes.len(), 55);
    }

    #[test]
    fn i18n_nearest_language() {
        for (tag, want) in [
            ("en_US.UTF-8", "en-CA"),
            ("en-GB", "en-GB"),
            ("en-IE", "en-GB"),
            ("en-AU", "en-AU"),
            ("da_DK.UTF-8", "da"),
            ("no_NO", "nb"),
            ("de-LI", "de-CH"),
            ("de-IT", "de-AT"),
            ("fr-CA", "fr-CA"),
            ("es-ES", "es-AR"),
            ("es-MX", "es-MX"),
            ("es-419", "es-CL"),
            ("sk", "cs"),
            ("zh-Hant-HK", "zh-TW"),
            ("zh-CN", "zh-SG"),
            ("prs", "fa"),
            ("iw", "he"),
            ("tlh", "en-GB"),
        ] {
            assert_eq!(nearest(tag), want, "{tag}");
        }
        for l in LANGUAGES {
            assert_eq!(nearest(l.code), l.code);
        }
    }

    #[test]
    fn i18n_variants_fall_back_to_their_base() {
        assert_eq!(chain("de-AT"), ["de-AT", "de", REFERENCE]);
        assert_eq!(chain("en-AU"), ["en-AU", REFERENCE]);
        assert_eq!(chain("es-MX"), ["es-MX", "es-CL", "es-AR", REFERENCE]);
        assert_eq!(chain("fr"), ["fr", REFERENCE]);
        let n = catalogue(REFERENCE).len();
        for l in LANGUAGES {
            assert_eq!(catalogue(l.code).len(), n, "{}", l.code);
        }
        let de_ch = catalogue("de-CH");
        for (key, v) in &de_ch {
            assert!(!v.to_string().contains(['ß', 'ẞ']), "de-CH: {key}: {v}");
        }
    }

    #[test]
    fn i18n_rtl_and_digits() {
        let rtl: Vec<&str> = LANGUAGES.iter().filter(|l| l.rtl).map(|l| l.code).collect();
        assert_eq!(rtl, ["he", "fa"]);
        assert!(is_rtl("fa") && is_rtl("he") && !is_rtl("hy") && !is_rtl("nope"));
        assert_eq!(digits_in("fa", "2026-10-06".into()), "۲۰۲۶-۱۰-۰۶");
        assert_eq!(digits_in("ka", "2026".into()), "2026");
    }

    #[test]
    fn i18n_sizes_in_each_languages_numbers() {
        assert_eq!(bytes_in("en-GB", 999), "999 B");
        assert_eq!(bytes_in("en-GB", 1_500_000_000), "1.5 GB");
        assert_eq!(bytes_in("da", 1_500_000_000), "1,5 GB");
        assert_eq!(bytes_in("de-AT", 2_260_000), "2,3 MB");
        assert_eq!(bytes_in("de-CH", 1_500_000_000), "1.5 GB", "Swiss German writes a point");
        assert_eq!(bytes_in("es-MX", 1_500_000_000), "1.5 GB");
        assert_eq!(bytes_in("es-AR", 1_500_000_000), "1,5 GB");
        assert_eq!(bytes_in("ja", 1_500_000_000), "1.5 GB");
        assert_eq!(bytes_in("fa", 1_500_000_000), "۱٫۵ GB");
        assert_eq!(bytes_in("fr", 3_000_000_000_000), "3,0 TB");
    }

    #[test]
    fn i18n_plural_categories() {
        assert_eq!(
            [0, 1, 2, 5].map(|n| plural("en-GB", n)),
            ["other", "one", "other", "other"]
        );
        assert_eq!(
            [0, 1, 2, 1_000_000].map(|n| plural("fr", n)),
            ["one", "one", "other", "many"]
        );
        assert_eq!(
            [1, 2, 5, 11, 21].map(|n| plural("lt", n)),
            ["one", "few", "few", "other", "one"]
        );
        assert_eq!(
            [0, 1, 11, 21].map(|n| plural("lv", n)),
            ["zero", "one", "zero", "one"]
        );
        assert_eq!([1, 2, 3].map(|n| plural("he", n)), ["one", "two", "other"]);
        assert_eq!(
            [1, 2, 5, 21].map(|n| plural("uk", n)),
            ["one", "few", "many", "one"]
        );
        assert_eq!([1, 2, 5].map(|n| plural("cs", n)), ["one", "few", "other"]);
        assert_eq!(
            [0, 1, 2].map(|n| plural("ja", n)),
            ["other", "other", "other"]
        );
        assert_eq!([0, 1, 2].map(|n| plural("fa", n)), ["one", "one", "other"]);
    }

    #[test]
    fn i18n_tr_fills_placeholders_and_shows_a_missing_key() {
        assert_eq!(tr_in(REFERENCE, "settings.title"), "Settings");
        assert_eq!(tr("no.such.key", &[]), "no.such.key");
        assert_eq!(fill("{a} and {b}", &[("a", "1"), ("b", "2")]), "1 and 2");
    }

    /// Every `t!("key")` and `tr("key", …)` in the workspace's Rust, and `t("key")` in the
    /// desktop app, names a text that exists.
    #[test]
    fn i18n_every_text_the_code_names_is_there() {
        use std::path::{Path, PathBuf};
        fn walk(d: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, exts, out);
                } else if p.extension().is_some_and(|x| exts.iter().any(|e| x == *e)) {
                    out.push(p);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = vec![];
        for c in std::fs::read_dir(root.join("crates"))
            .into_iter()
            .flatten()
            .flatten()
        {
            walk(&c.path().join("src"), &["rs"], &mut files);
        }
        walk(&root.join("gui/src"), &["js", "svelte", "ts"], &mut files);
        let reference = catalogue(REFERENCE);
        let mut bad = vec![];
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap_or_default();
            let rust = f.extension().is_some_and(|x| x == "rs");
            let calls: &[&str] = if rust {
                &["t!(", "tn!(", "tr(", "trn("]
            } else {
                &["t(", "tn("]
            };
            for call in calls {
                for (i, _) in text.match_indices(call) {
                    if text[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.') {
                        continue;
                    }
                    let line = &text[text[..i].rfind('\n').map_or(0, |p| p + 1)..i];
                    if line.contains("//") || line.trim_start().starts_with('*') {
                        continue;
                    }
                    let rest = text[i + call.len()..].trim_start();
                    let Some(q) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                        continue;
                    };
                    let Some(end) = rest[1..].find(q) else {
                        continue;
                    };
                    let key = &rest[1..1 + end];
                    if !key.contains('.')
                        || key.ends_with('.')
                        || key.contains([' ', '{', '$'])
                        || key.starts_with("no.")
                    {
                        continue;
                    }
                    if !reference.contains_key(key) {
                        bad.push(format!("{}: no text {key}", f.display()));
                    }
                }
            }
        }
        assert!(files.len() > 5, "the sources are where this test looks");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A language of its own has every text British English has, with the same placeholders;
    /// a variant holds only keys the reference knows.
    #[test]
    fn i18n_catalogues_are_complete_and_well_formed() {
        let reference = catalogue(REFERENCE);
        assert!(!reference.is_empty());
        let placeholders = |s: &str| {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|x| x.0.to_string()))
                .collect();
            v.retain(|p| p != "n");
            v.sort();
            v
        };
        let texts = |v: &serde_json::Value| -> Vec<String> {
            match v {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Object(o) => o
                    .values()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect(),
                _ => vec![],
            }
        };
        for code in LANGUAGES.iter().map(|l| l.code) {
            let own: Map =
                serde_json::from_str(source(code)).unwrap_or_else(|e| panic!("{code}.json: {e}"));
            if !code.starts_with("en") && chain(code).len() == 2 {
                let mut missing: Vec<&String> =
                    reference.keys().filter(|k| !own.contains_key(*k)).collect();
                missing.sort();
                assert!(missing.is_empty(), "{code} lacks {missing:?}");
            }
            for (key, v) in &own {
                let Some(r) = reference.get(key) else {
                    panic!("{code}: unknown key {key}")
                };
                let want = texts(r)
                    .first()
                    .map(|s| placeholders(s))
                    .unwrap_or_default();
                for t in texts(v) {
                    assert_eq!(placeholders(&t), want, "{code}: {key}: {t}");
                }
                if let serde_json::Value::Object(o) = v {
                    assert!(o.contains_key("other"), "{code}: {key} lacks \"other\"");
                }
            }
        }
    }

    /// Each language names one thing one way: the variants a file once mixed stay out, and a
    /// file writes its apostrophes inside words either straight or curly, not both.
    #[test]
    fn i18n_one_term_for_one_thing() {
        // (language, word prefixes it no longer uses: the term it chose is in the comment)
        const NOT: &[(&str, &[&str])] = &[
            ("fi", &["levykuv", "säiliö"]), // konttikuva, kontti
            ("et", &["seade"]),             // sätted
            ("lt", &["parametrai"]),        // nustatymai
            ("da", &["aftryk"]),            // image
            ("nl", &["u ", "uw "]),         // je, jouw
        ];
        let words = |s: &str| -> Vec<String> {
            s.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’').map(|w| w.to_lowercase()).collect()
        };
        let mut bad = vec![];
        for code in LANGUAGES.iter().map(|l| l.code) {
            let own: Map = serde_json::from_str(source(code)).unwrap();
            let texts: Vec<(String, String)> = own
                .iter()
                .flat_map(|(k, v)| match v {
                    serde_json::Value::String(s) => vec![(k.clone(), s.clone())],
                    serde_json::Value::Object(o) => o.values().filter_map(|x| x.as_str()).map(|s| (k.clone(), s.to_string())).collect(),
                    _ => vec![],
                })
                .collect();
            for (lang, not) in NOT.iter().filter(|(l, _)| *l == code) {
                for (k, t) in &texts {
                    for n in *not {
                        let hit = match n.strip_suffix(' ') {
                            Some(whole) => words(t).iter().any(|w| w == whole),
                            None => words(t).iter().any(|w| w.starts_with(n)),
                        };
                        if hit {
                            bad.push(format!("{lang}: {k} says {n:?}: {t}"));
                        }
                    }
                }
            }
            let inside = |q: char| -> Vec<&String> {
                texts.iter().filter(|(_, t)| t.chars().collect::<Vec<_>>().windows(3).any(|w| w[0].is_alphabetic() && w[1] == q && w[2].is_alphabetic())).map(|(k, _)| k).collect()
            };
            let (straight, curly) = (inside('\''), inside('’'));
            if !straight.is_empty() && !curly.is_empty() {
                bad.push(format!("{code}: apostrophes both straight ({straight:?}) and curly ({curly:?})"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
