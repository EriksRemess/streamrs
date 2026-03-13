use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Command, Stdio};

pub fn launch_split_command(command_line: &str, debug: bool) -> Result<(), String> {
    let parts: Vec<&str> = command_line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let mut cmd = Command::new(parts[0]);
    cmd.args(&parts[1..]).stdin(Stdio::null());

    if debug {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Error launching '{command_line}': {e}"))
}

pub fn run_shell_status(command: &str) -> Result<bool, String> {
    if command.trim().is_empty() {
        return Err("Status check command is empty".to_string());
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| format!("Failed to run status check '{command}': {err}"))?;

    Ok(status.success())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedShortcut {
    modifiers: Vec<Key>,
    trigger: Key,
}

pub fn send_keyboard_shortcut(shortcut: &str) -> Result<(), String> {
    let parsed = parse_keyboard_shortcut(shortcut)?;
    eprintln!("Sending keyboard shortcut '{shortcut}'");
    catch_unwind(AssertUnwindSafe(|| {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|err| format!("Failed to initialize keyboard input backend: {err}"))?;

        for key in &parsed.modifiers {
            enigo
                .key(*key, Press)
                .map_err(|err| format!("Failed to press modifier for '{shortcut}': {err}"))?;
        }

        let trigger_result = enigo
            .key(parsed.trigger, Click)
            .map_err(|err| format!("Failed to send keyboard shortcut '{shortcut}': {err}"));

        for key in parsed.modifiers.iter().rev() {
            let _ = enigo.key(*key, Release);
        }

        trigger_result
    }))
    .map_err(|_| format!("Keyboard input backend panicked while sending shortcut '{shortcut}'"))??;
    eprintln!("Keyboard shortcut '{shortcut}' dispatched");
    Ok(())
}

fn parse_keyboard_shortcut(shortcut: &str) -> Result<ParsedShortcut, String> {
    let tokens: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err("Keyboard shortcut is empty".to_string());
    }

    let mut modifiers = Vec::new();
    for token in &tokens[..tokens.len().saturating_sub(1)] {
        let key = parse_modifier_token(token).ok_or_else(|| {
            format!("Unsupported modifier '{token}' in keyboard shortcut '{shortcut}'")
        })?;
        modifiers.push(key);
    }

    let trigger = parse_key_token(tokens[tokens.len() - 1]).ok_or_else(|| {
        format!(
            "Unsupported trigger key '{}' in keyboard shortcut '{}'",
            tokens[tokens.len() - 1],
            shortcut
        )
    })?;

    Ok(ParsedShortcut { modifiers, trigger })
}

fn parse_modifier_token(token: &str) -> Option<Key> {
    match normalize_shortcut_token(token).as_str() {
        "ctrl" | "control" => Some(Key::Control),
        "shift" => Some(Key::Shift),
        "alt" | "option" => Some(Key::Alt),
        "meta" | "super" | "win" | "windows" | "cmd" | "command" => Some(Key::Meta),
        _ => None,
    }
}

fn parse_key_token(token: &str) -> Option<Key> {
    if let Some(key) = parse_modifier_token(token) {
        return Some(key);
    }

    let normalized = normalize_shortcut_token(token);
    match normalized.as_str() {
        "enter" | "return" => Some(Key::Return),
        "tab" => Some(Key::Tab),
        "space" | "spacebar" => Some(Key::Space),
        "esc" | "escape" => Some(Key::Escape),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "insert" | "ins" => Some(Key::Insert),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" | "pgup" => Some(Key::PageUp),
        "pagedown" | "pgdn" => Some(Key::PageDown),
        "up" | "uparrow" => Some(Key::UpArrow),
        "down" | "downarrow" => Some(Key::DownArrow),
        "left" | "leftarrow" => Some(Key::LeftArrow),
        "right" | "rightarrow" => Some(Key::RightArrow),
        "capslock" => Some(Key::CapsLock),
        "printscreen" | "prtsc" | "prtscr" | "printscr" => Some(Key::PrintScr),
        "pause" => Some(Key::Pause),
        "scrolllock" => Some(Key::ScrollLock),
        "numlock" => Some(Key::Numlock),
        "menu" => Some(Key::LMenu),
        "plus" => Some(Key::Unicode('+')),
        "minus" => Some(Key::Unicode('-')),
        "comma" => Some(Key::Unicode(',')),
        "period" | "dot" => Some(Key::Unicode('.')),
        "slash" | "forwardslash" => Some(Key::Unicode('/')),
        "backslash" => Some(Key::Unicode('\\')),
        "semicolon" => Some(Key::Unicode(';')),
        "quote" | "apostrophe" => Some(Key::Unicode('\'')),
        "backtick" | "grave" => Some(Key::Unicode('`')),
        "leftbracket" | "lbracket" => Some(Key::Unicode('[')),
        "rightbracket" | "rbracket" => Some(Key::Unicode(']')),
        _ => parse_function_key(&normalized).or_else(|| parse_single_character_key(token)),
    }
}

fn parse_function_key(token: &str) -> Option<Key> {
    let number = token.strip_prefix('f')?.parse::<u8>().ok()?;
    match number {
        1 => Some(Key::F1),
        2 => Some(Key::F2),
        3 => Some(Key::F3),
        4 => Some(Key::F4),
        5 => Some(Key::F5),
        6 => Some(Key::F6),
        7 => Some(Key::F7),
        8 => Some(Key::F8),
        9 => Some(Key::F9),
        10 => Some(Key::F10),
        11 => Some(Key::F11),
        12 => Some(Key::F12),
        13 => Some(Key::F13),
        14 => Some(Key::F14),
        15 => Some(Key::F15),
        16 => Some(Key::F16),
        17 => Some(Key::F17),
        18 => Some(Key::F18),
        19 => Some(Key::F19),
        20 => Some(Key::F20),
        21 => Some(Key::F21),
        22 => Some(Key::F22),
        23 => Some(Key::F23),
        24 => Some(Key::F24),
        25 => Some(Key::F25),
        26 => Some(Key::F26),
        27 => Some(Key::F27),
        28 => Some(Key::F28),
        29 => Some(Key::F29),
        30 => Some(Key::F30),
        31 => Some(Key::F31),
        32 => Some(Key::F32),
        33 => Some(Key::F33),
        34 => Some(Key::F34),
        35 => Some(Key::F35),
        _ => None,
    }
}

fn parse_single_character_key(token: &str) -> Option<Key> {
    let mut chars = token.trim().chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(Key::Unicode(first))
}

fn normalize_shortcut_token(token: &str) -> String {
    token
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_status_command_is_rejected() {
        let err = run_shell_status("   ").expect_err("blank status command should fail");
        assert!(err.contains("empty"));
    }

    #[test]
    fn shell_status_reports_success_and_failure() {
        assert!(run_shell_status("true").expect("true should run"));
        assert!(!run_shell_status("false").expect("false should run"));
    }

    #[test]
    fn empty_launch_command_is_noop() {
        launch_split_command("   ", false).expect("blank launch command should be a no-op");
    }

    #[test]
    fn launch_errors_are_reported() {
        let err = launch_split_command("streamrs-test-command-that-should-not-exist", false)
            .expect_err("missing executable should return an error");
        assert!(err.contains("Error launching"));
    }

    #[test]
    fn keyboard_shortcut_parser_handles_common_shortcuts() {
        assert_eq!(
            parse_keyboard_shortcut("Ctrl+Shift+T").expect("shortcut should parse"),
            ParsedShortcut {
                modifiers: vec![Key::Control, Key::Shift],
                trigger: Key::Unicode('T'),
            }
        );
        assert_eq!(
            parse_keyboard_shortcut("Meta+Return").expect("shortcut should parse"),
            ParsedShortcut {
                modifiers: vec![Key::Meta],
                trigger: Key::Return,
            }
        );
        assert_eq!(
            parse_keyboard_shortcut("Ctrl+Alt+Delete").expect("shortcut should parse"),
            ParsedShortcut {
                modifiers: vec![Key::Control, Key::Alt],
                trigger: Key::Delete,
            }
        );
    }

    #[test]
    fn keyboard_shortcut_parser_rejects_invalid_modifier_positions() {
        let err =
            parse_keyboard_shortcut("Ctrl+Hello+T").expect_err("invalid modifier should fail");
        assert!(err.contains("Unsupported modifier"));
    }

    #[test]
    fn keyboard_shortcut_parser_rejects_blank_input() {
        let err = parse_keyboard_shortcut("   ").expect_err("blank shortcut should fail");
        assert!(err.contains("empty"));
    }
}
