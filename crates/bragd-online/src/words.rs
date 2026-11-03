//! The word filter for a Chat with a stranger: a word on the list (`words.txt`, one lower-case
//! word a line, `#` for a comment) is shown as asterisks, one for each of its letters. Whole
//! words only, whatever their case, so a town such as Scunthorpe keeps its name. It runs where the
//! line is shown, on the reader's machine, and the reader can turn it off in Settings → Online.
//!
//! shortcut: a short list in a few languages; a fuller one per locale when users ask for it.

use std::collections::HashSet;
use std::sync::LazyLock;

static WORDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    include_str!("words.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_lowercase())
        .collect()
});

/// `text` with every listed word masked.
pub fn mask(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphanumeric() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        if WORDS.contains(&word.to_lowercase()) {
            out.push_str(&"*".repeat(i - start));
        } else {
            out.push_str(&word);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::mask;

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
    fn words_the_list_is_lower_case_and_has_no_blanks() {
        for line in include_str!("words.txt").lines().filter(|l| !l.starts_with('#')) {
            assert!(!line.trim().is_empty() && line == line.trim() && line == line.to_lowercase(), "{line:?}");
        }
    }
}
