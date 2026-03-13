use super::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

const GETTEXT_DOMAIN: &str = "streamrs";
const LC_ALL: c_int = 6;

unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    fn bindtextdomain(domainname: *const c_char, dirname: *const c_char) -> *mut c_char;
    fn textdomain(domainname: *const c_char) -> *mut c_char;
    fn gettext(msgid: *const c_char) -> *mut c_char;
}

fn cstring(value: &str) -> Option<CString> {
    CString::new(value).ok()
}

fn locale_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("STREAMRS_LOCALEDIR")
        && !path.is_empty()
    {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(exe) = env::current_exe()
        && let Some(prefix) = exe.parent().and_then(Path::parent)
    {
        candidates.push(prefix.join("share").join("locale"));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("po")
            .join("locale"),
    );
    candidates.push(PathBuf::from("/usr/share/locale"));
    candidates
}

fn resolve_locale_dir() -> PathBuf {
    locale_dir_candidates()
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("/usr/share/locale"))
}

pub(crate) fn init_i18n() {
    let Some(empty) = cstring("") else {
        return;
    };
    let Some(domain) = cstring(GETTEXT_DOMAIN) else {
        return;
    };
    let Some(locale_dir) = resolve_locale_dir().to_str().and_then(cstring) else {
        return;
    };

    unsafe {
        setlocale(LC_ALL, empty.as_ptr());
        bindtextdomain(domain.as_ptr(), locale_dir.as_ptr());
        textdomain(domain.as_ptr());
    }
}

pub(crate) fn tr(msgid: &str) -> String {
    let original = msgid.to_string();
    let Some(msgid) = cstring(msgid) else {
        return original;
    };

    unsafe {
        let translated = gettext(msgid.as_ptr());
        if translated.is_null() {
            return original;
        }
        CStr::from_ptr(translated).to_string_lossy().into_owned()
    }
}

pub(crate) fn trf(msgid: &str, replacements: &[(&str, String)]) -> String {
    let mut rendered = tr(msgid);
    for (key, value) in replacements {
        rendered = rendered.replace(&format!("{{{key}}}"), value);
    }
    rendered
}

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
