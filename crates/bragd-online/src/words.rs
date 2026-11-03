//! The word filter for a Chat with a stranger: a word on a list (`words/<language>.txt`, one word
//! a line, `#` for a comment) is shown as asterisks, one for each of its characters. Every list
//! applies to every line, since a stranger may write in any language. Words are compared folded:
//! lower case, without accents, with full-width letters made plain and with the common digit and
//! symbol stand-ins (`sh1t`, `$hit`) read as letters. Whole words only, so a town such as
//! Scunthorpe keeps its name, except in scripts written without spaces (Chinese, Japanese, Korean
//! syllables), where a listed word is found inside a longer run. It runs where the line is shown,
//! on the reader's machine, and the reader can turn it off in Settings → Online.
//!
//! shortcut: no list yet for some of Bragd's languages (#337); add `words/<language>.txt`.

use std::collections::HashSet;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Every list, by language code (the part of a locale before `-`).
const LISTS: &[(&str, &str)] = &[
    ("cs", include_str!("words/cs.txt")),
    ("da", include_str!("words/da.txt")),
    ("de", include_str!("words/de.txt")),
    ("en", include_str!("words/en.txt")),
    ("es", include_str!("words/es.txt")),
    ("fa", include_str!("words/fa.txt")),
    ("fi", include_str!("words/fi.txt")),
    ("fr", include_str!("words/fr.txt")),
    ("it", include_str!("words/it.txt")),
    ("ja", include_str!("words/ja.txt")),
    ("ko", include_str!("words/ko.txt")),
    ("nl", include_str!("words/nl.txt")),
    ("no", include_str!("words/no.txt")),
    ("pl", include_str!("words/pl.txt")),
    ("sv", include_str!("words/sv.txt")),
    ("zh", include_str!("words/zh.txt")),
];

struct Words {
    /// Folded words matched against a whole word.
    whole: HashSet<String>,
    /// Folded words in a script without spaces, matched inside a word.
    inside: Vec<String>,
}

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    let mut words = Words { whole: HashSet::new(), inside: Vec::new() };
    for word in LISTS.iter().flat_map(|(_, list)| entries(list)) {
        let folded: String = fold(&word.chars().collect::<Vec<_>>()).into_iter().map(|(c, _)| c).collect();
        if word.chars().any(unspaced) {
            words.inside.push(folded);
        } else {
            words.whole.insert(folded);
        }
    }
    words
});

fn entries(list: &str) -> impl Iterator<Item = &str> {
    list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
}

/// A character that can be part of a word, including the symbols that stand in for letters.
fn in_word(c: char) -> bool {
    c.is_alphanumeric() || is_combining_mark(c) || c == '@' || c == '$'
}

/// Han, kana and Hangul: scripts whose words are not set apart by spaces.
fn unspaced(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x11FF | 0x3040..=0x30FF | 0x3130..=0x318F | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0x20000..=0x3FFFF)
}

/// The folded characters of a word, each with the index of the character it came from.
fn fold(word: &[char]) -> Vec<(char, usize)> {
    // Digits stand for letters only next to letters, so `1500` or `1-0` stay what they are.
    let leet = word.iter().any(|c| c.is_alphabetic());
    let mut out = Vec::new();
    for (i, &c) in word.iter().enumerate() {
        let c = match c {
            '0' if leet => 'o',
            '1' if leet => 'i',
            '3' if leet => 'e',
            '4' if leet => 'a',
            '@' => 'a',
            '5' | '$' if leet => 's',
            '7' if leet => 't',
            _ => c,
        };
        for f in c.to_lowercase().collect::<String>().nfkd().filter(|f| !is_combining_mark(*f)) {
            out.extend(f.to_lowercase().map(|f| (f, i)));
        }
    }
    out
}

/// `text` with every listed word masked.
pub fn mask(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut hide = vec![false; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if !in_word(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && in_word(chars[i]) {
            i += 1;
        }
        let folded = fold(&chars[start..i]);
        let s: String = folded.iter().map(|(c, _)| c).collect();
        if WORDS.whole.contains(&s) {
            hide[start..i].fill(true);
            continue;
        }
        for word in &WORDS.inside {
            for (at, _) in s.match_indices(word.as_str()) {
                let first = s[..at].chars().count();
                let last = first + word.chars().count() - 1;
                hide[start + folded[first].1..=start + folded[last].1].fill(true);
            }
        }
    }
    chars.iter().zip(hide).map(|(&c, h)| if h { '*' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::{LISTS, entries, in_word, mask};

    #[test]
    fn words_masks_listed_words_whatever_their_case() {
        assert_eq!(mask("what the fuck"), "what the ****");
        assert_eq!(mask("FUCK this"), "**** this");
        assert_eq!(mask("Shit!"), "****!");
        assert_eq!(mask("good game, well played"), "good game, well played");
    }

    #[test]
    fn words_masks_whole_words_only() {
        assert_eq!(mask("Scunthorpe and Penistone"), "Scunthorpe and Penistone");
        assert_eq!(mask("cunt"), "****");
        assert_eq!(mask("shitake"), "shitake");
    }

    #[test]
    fn words_counts_letters_not_bytes() {
        // Danish: a word with æ keeps one asterisk a letter, and the text around it is unchanged.
        assert_eq!(mask("din røvhul!"), "din ******!");
        assert_eq!(mask("ÆØÅ"), "ÆØÅ");
    }

    #[test]
    fn words_sees_through_accents_stand_ins_and_full_width_letters() {
        assert_eq!(mask("sh1t"), "****");
        assert_eq!(mask("$hit happens"), "**** happens");
        assert_eq!(mask("a55hole"), "*******");
        assert_eq!(mask("FÜCK"), "****");
        assert_eq!(mask("ｆｕｃｋ"), "****");
        assert_eq!(mask("cabron y cabrón"), "****** y ******");
        // Decomposed accents count as one character with their letter.
        assert_eq!(mask("encule\u{301}!"), "*******!");
    }

    #[test]
    fn words_leaves_moves_scores_and_numbers_alone() {
        let game = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O, 1-0, 1500, 5:30";
        assert_eq!(mask(game), game);
    }

    #[test]
    fn words_masks_every_language_but_not_everyday_words_of_another() {
        assert_eq!(mask("vaffanculo, kurwa, vittu"), "**********, *****, *****");
        // Words that are rude in one language and everyday in another stay.
        let everyday = "bitte, con el negro, la regina, les negres, sex brikker, dicke Luft, \
                        désolé du retard, mutta, fick, perse";
        assert_eq!(mask(everyday), everyday);
    }

    #[test]
    fn words_finds_a_word_inside_a_run_of_han_or_hangul() {
        assert_eq!(mask("你他妈的"), "你***");
        assert_eq!(mask("이 씨발놈아"), "이 ***아");
        // A run without a listed word is left as it is: 妈妈的 is "mum's", not 他妈的.
        assert_eq!(mask("我妈妈的书, 可能性"), "我妈妈的书, 可能性");
    }

    #[test]
    fn words_every_list_is_one_word_a_line_for_a_language_bragd_ships() {
        let locales = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../bragd-core/locales"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        for (lang, list) in LISTS {
            // Norwegian's list serves both nb and nn.
            let lang = if *lang == "no" { "nb" } else { lang };
            assert!(locales.iter().any(|l| l.split(['-', '.']).next() == Some(lang)), "{lang}");
            for word in entries(list) {
                assert!(word.chars().all(in_word) && word == word.to_lowercase(), "{lang}: {word:?}");
            }
        }
    }
}
