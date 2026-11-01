//! A Comment's language, guessed on the machine (whatlang), so one already in the reader's
//! language is never sent to a model.

use whatlang::Lang;

/// `l`'s two-letter ISO 639-1 code (`Lang::Eng` → `"en"`), as Bragd's locales name languages.
/// Norwegian Bokmål is `"nb"`. A language with no two-letter code gives its three-letter one.
pub fn iso1(l: Lang) -> &'static str {
    match l {
        Lang::Afr => "af",
        Lang::Aka => "ak",
        Lang::Amh => "am",
        Lang::Ara => "ar",
        Lang::Aze => "az",
        Lang::Bel => "be",
        Lang::Ben => "bn",
        Lang::Bul => "bg",
        Lang::Cat => "ca",
        Lang::Ces => "cs",
        Lang::Cmn => "zh",
        Lang::Cym => "cy",
        Lang::Dan => "da",
        Lang::Deu => "de",
        Lang::Ell => "el",
        Lang::Eng => "en",
        Lang::Epo => "eo",
        Lang::Est => "et",
        Lang::Fin => "fi",
        Lang::Fra => "fr",
        Lang::Guj => "gu",
        Lang::Heb => "he",
        Lang::Hin => "hi",
        Lang::Hrv => "hr",
        Lang::Hun => "hu",
        Lang::Hye => "hy",
        Lang::Ind => "id",
        Lang::Ita => "it",
        Lang::Jav => "jv",
        Lang::Jpn => "ja",
        Lang::Kan => "kn",
        Lang::Kat => "ka",
        Lang::Khm => "km",
        Lang::Kor => "ko",
        Lang::Lat => "la",
        Lang::Lav => "lv",
        Lang::Lit => "lt",
        Lang::Mal => "ml",
        Lang::Mar => "mr",
        Lang::Mkd => "mk",
        Lang::Mya => "my",
        Lang::Nep => "ne",
        Lang::Nld => "nl",
        Lang::Nob => "nb",
        Lang::Ori => "or",
        Lang::Pan => "pa",
        Lang::Pes => "fa",
        Lang::Pol => "pl",
        Lang::Por => "pt",
        Lang::Ron => "ro",
        Lang::Rus => "ru",
        Lang::Sin => "si",
        Lang::Slk => "sk",
        Lang::Slv => "sl",
        Lang::Sna => "sn",
        Lang::Spa => "es",
        Lang::Srp => "sr",
        Lang::Swe => "sv",
        Lang::Tam => "ta",
        Lang::Tel => "te",
        Lang::Tgl => "tl",
        Lang::Tha => "th",
        Lang::Tuk => "tk",
        Lang::Tur => "tr",
        Lang::Ukr => "uk",
        Lang::Urd => "ur",
        Lang::Uzb => "uz",
        Lang::Vie => "vi",
        Lang::Yid => "yi",
        Lang::Zul => "zu",
    }
}

/// The language `text` is in, by its two-letter code, when the guess is reliable.
pub fn guess(text: &str) -> Option<&'static str> {
    whatlang::detect(text).filter(|info| info.is_reliable()).map(|info| iso1(info.lang()))
}

/// Whether `text` is reliably in the language of the locale `reader` (`"da"`, `"en-AU"`,
/// `"nn"`): its first subtag matches, and Bokmål and Nynorsk count as one.
pub fn in_language(text: &str, reader: &str) -> bool {
    let tag = reader.split_once('-').map_or(reader, |(tag, _)| tag);
    let tag = if tag == "nn" { "nb" } else { tag };
    guess(text) == Some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_every_language_has_a_code_of_two_or_three_letters() {
        for &l in Lang::all() {
            let c = iso1(l);
            assert!(c.len() == 2 || c == l.code(), "{l:?}: {c}");
        }
        assert_eq!(iso1(Lang::Eng), "en");
        assert_eq!(iso1(Lang::Dan), "da");
        assert_eq!(iso1(Lang::Deu), "de");
        assert_eq!(iso1(Lang::Nob), "nb");
        assert_eq!(iso1(Lang::Cmn), "zh");
        assert_eq!(iso1(Lang::Jpn), "ja");
        assert_eq!(iso1(Lang::Heb), "he");
        assert_eq!(iso1(Lang::Pes), "fa");
        assert_eq!(iso1(Lang::Ukr), "uk");
    }

    #[test]
    fn lang_guesses_a_sentence_and_not_a_scrap() {
        assert_eq!(guess("Weiß hätte hier besser den Springer ziehen sollen, denn danach ist die Stellung verloren."), Some("de"));
        assert_eq!(guess("White should have moved the knight here, since after this the position is lost."), Some("en"));
        assert_eq!(guess("+-"), None);
    }

    #[test]
    fn lang_in_language_compares_the_first_subtag() {
        let en = "White should have moved the knight here, since after this the position is lost.";
        assert!(in_language(en, "en-AU"));
        assert!(in_language(en, "en-GB"));
        assert!(!in_language(en, "da"));
        let de = "Weiß hätte hier besser den Springer ziehen sollen, denn danach ist die Stellung verloren.";
        assert!(in_language(de, "de-AT"));
        assert!(!in_language("+-", "en-GB"), "a scrap is not known to be in any language");
    }
}
