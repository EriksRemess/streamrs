use super::*;
pub(crate) use streamrs::i18n::{init_i18n, tr, trf};

fn current_language_tag() -> Option<String> {
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(value) = env::var(key) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let language = value.split(':').next().unwrap_or(value);
        let language = language
            .split('.')
            .next()
            .unwrap_or(language)
            .split('@')
            .next()
            .unwrap_or(language);
        if !language.is_empty() {
            return Some(language.to_string());
        }
    }
    None
}

fn english_ordinal(number: usize) -> String {
    let suffix = match number % 100 {
        11..=13 => "th",
        _ => match number % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{number}{suffix}")
}

pub(crate) fn tr_ordinal(number: usize) -> String {
    match current_language_tag().as_deref() {
        Some(language) if language.starts_with("en") => english_ordinal(number),
        _ => format!("{number}."),
    }
}
