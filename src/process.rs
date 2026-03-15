use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use gtk::{
    gio::{self, prelude::*},
    glib::{
        self,
        variant::{FromVariant, ObjectPath, ToVariant},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
#[cfg(unix)]
use std::fs::Permissions;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use xkeysym::key as keysym;

const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_REMOTE_DESKTOP_INTERFACE: &str = "org.freedesktop.portal.RemoteDesktop";
const PORTAL_REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const PORTAL_SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";
const PORTAL_DEVICE_TYPE_KEYBOARD: u32 = 1;
const PORTAL_KEY_RELEASED: u32 = 0;
const PORTAL_KEY_PRESSED: u32 = 1;
const PORTAL_RESPONSE_SUCCESS: u32 = 0;
const PORTAL_RESPONSE_CANCELLED: u32 = 1;
const PORTAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const EMPTY_WINDOW_ID: &str = "";
const PORTAL_PERSIST_MODE_PERSISTENT: u32 = 2;

static PORTAL_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
static PORTAL_KEYBOARD_BACKEND: OnceLock<Mutex<Option<PortalKeyboardBackend>>> = OnceLock::new();

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
    modifiers: Vec<ShortcutKey>,
    trigger: ShortcutKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutKey {
    Control,
    Shift,
    Alt,
    Meta,
    Return,
    Tab,
    Space,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    CapsLock,
    PrintScreen,
    Pause,
    ScrollLock,
    NumLock,
    Menu,
    Function(u8),
    Character(char),
}

struct PortalKeyboardBackend {
    proxy: gio::DBusProxy,
    session_path: ObjectPath,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct StreamrsState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    portal_remote_desktop_restore_token: Option<String>,
}

impl PortalKeyboardBackend {
    fn new() -> Result<Self, String> {
        let connection =
            gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).map_err(|err| {
                format!("Failed to connect to session bus for portal keyboard input: {err}")
            })?;
        let proxy = gio::DBusProxy::new_sync(
            &connection,
            gio::DBusProxyFlags::NONE,
            None::<&gio::DBusInterfaceInfo>,
            Some(PORTAL_BUS_NAME),
            PORTAL_DESKTOP_PATH,
            PORTAL_REMOTE_DESKTOP_INTERFACE,
            gio::Cancellable::NONE,
        )
        .map_err(|err| format!("Failed to create RemoteDesktop portal proxy: {err}"))?;

        let available_device_types = proxy
            .cached_property("AvailableDeviceTypes")
            .and_then(|value| u32::from_variant(&value))
            .ok_or_else(|| {
                "RemoteDesktop portal did not expose AvailableDeviceTypes".to_string()
            })?;
        if available_device_types & PORTAL_DEVICE_TYPE_KEYBOARD == 0 {
            return Err(
                "RemoteDesktop portal does not advertise keyboard injection support".to_string(),
            );
        }
        let portal_version = proxy
            .cached_property("version")
            .and_then(|value| u32::from_variant(&value))
            .unwrap_or(1);
        let persistence_supported = portal_version >= 2;
        let restore_token = persistence_supported
            .then(load_portal_restore_token)
            .transpose()?
            .flatten();

        let session_token = next_portal_token("streamrs_session");
        let create_request_token = next_portal_token("streamrs_request");
        let create_request_path = portal_request_path(&connection, &create_request_token)?;
        let create_options = portal_options(&[
            ("handle_token", create_request_token.to_variant()),
            ("session_handle_token", session_token.to_variant()),
        ]);

        let create_results = wait_for_portal_response(&connection, &create_request_path, || {
            proxy
                .call_sync(
                    "CreateSession",
                    Some(&(create_options.clone(),).to_variant()),
                    gio::DBusCallFlags::NONE,
                    -1,
                    gio::Cancellable::NONE,
                )
                .map(|_| ())
                .map_err(|err| format!("RemoteDesktop CreateSession failed: {err}"))
        })?;
        let session_path = create_results
            .get("session_handle")
            .and_then(String::from_variant)
            .ok_or_else(|| {
                "RemoteDesktop CreateSession response did not include session_handle".to_string()
            })
            .and_then(|path| {
                ObjectPath::try_from(path)
                    .map_err(|err| format!("Portal returned invalid session_handle: {err}"))
            })?;

        let select_request_token = next_portal_token("streamrs_request");
        let select_request_path = portal_request_path(&connection, &select_request_token)?;
        let mut select_options = portal_options(&[
            ("handle_token", select_request_token.to_variant()),
            ("types", PORTAL_DEVICE_TYPE_KEYBOARD.to_variant()),
        ]);
        if persistence_supported {
            select_options.insert(
                "persist_mode".to_string(),
                PORTAL_PERSIST_MODE_PERSISTENT.to_variant(),
            );
            if let Some(ref restore_token) = restore_token {
                select_options.insert("restore_token".to_string(), restore_token.to_variant());
            }
        }

        wait_for_portal_response(&connection, &select_request_path, || {
            proxy
                .call_sync(
                    "SelectDevices",
                    Some(&(session_path.clone(), select_options.clone()).to_variant()),
                    gio::DBusCallFlags::NONE,
                    -1,
                    gio::Cancellable::NONE,
                )
                .map(|_| ())
                .map_err(|err| format!("RemoteDesktop SelectDevices failed: {err}"))
        })?;

        let start_request_token = next_portal_token("streamrs_request");
        let start_request_path = portal_request_path(&connection, &start_request_token)?;
        let start_options = portal_options(&[("handle_token", start_request_token.to_variant())]);

        let start_results = wait_for_portal_response(&connection, &start_request_path, || {
            proxy
                .call_sync(
                    "Start",
                    Some(
                        &(session_path.clone(), EMPTY_WINDOW_ID, start_options.clone())
                            .to_variant(),
                    ),
                    gio::DBusCallFlags::NONE,
                    -1,
                    gio::Cancellable::NONE,
                )
                .map(|_| ())
                .map_err(|err| format!("RemoteDesktop Start failed: {err}"))
        })?;
        if persistence_supported {
            match start_results
                .get("restore_token")
                .and_then(String::from_variant)
                .filter(|token| !token.trim().is_empty())
            {
                Some(token) => save_portal_restore_token(&token)?,
                None => clear_portal_restore_token()?,
            }
        }

        Ok(Self {
            proxy,
            session_path,
        })
    }

    fn send_shortcut(&self, parsed: &ParsedShortcut) -> Result<(), String> {
        for key in &parsed.modifiers {
            self.notify_key(*key, PORTAL_KEY_PRESSED)?;
        }

        let trigger_result = self
            .notify_key(parsed.trigger, PORTAL_KEY_PRESSED)
            .and_then(|_| self.notify_key(parsed.trigger, PORTAL_KEY_RELEASED));

        for key in parsed.modifiers.iter().rev() {
            let _ = self.notify_key(*key, PORTAL_KEY_RELEASED);
        }

        trigger_result
    }

    fn notify_key(&self, key: ShortcutKey, state: u32) -> Result<(), String> {
        let keysym = shortcut_key_to_keysym(key).ok_or_else(|| {
            format!("Keyboard shortcut key '{key:?}' is not supported by the portal backend")
        })?;
        let options = portal_options(&[]);
        self.proxy
            .call_sync(
                "NotifyKeyboardKeysym",
                Some(&(self.session_path.clone(), options, keysym, state).to_variant()),
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
            .map(|_| ())
            .map_err(|err| format!("RemoteDesktop NotifyKeyboardKeysym failed: {err}"))
    }
}

impl Drop for PortalKeyboardBackend {
    fn drop(&mut self) {
        let _ = self.proxy.connection().call_sync(
            Some(PORTAL_BUS_NAME),
            self.session_path.as_str(),
            PORTAL_SESSION_INTERFACE,
            "Close",
            None::<&glib::Variant>,
            None::<&glib::VariantTy>,
            gio::DBusCallFlags::NONE,
            -1,
            gio::Cancellable::NONE,
        );
    }
}

pub fn send_keyboard_shortcut(shortcut: &str) -> Result<(), String> {
    let parsed = parse_keyboard_shortcut(shortcut)?;
    eprintln!("Sending keyboard shortcut '{shortcut}'");
    let mut portal_error = None;

    if should_try_portal_keyboard() {
        match send_keyboard_shortcut_via_portal(&parsed) {
            Ok(()) => {
                eprintln!("Keyboard shortcut '{shortcut}' dispatched");
                return Ok(());
            }
            Err(err) => {
                eprintln!("Portal keyboard backend failed: {err}");
                portal_error = Some(err);
            }
        }
    }

    send_keyboard_shortcut_with_enigo(shortcut, &parsed).map_err(|err| {
        if let Some(portal_err) = portal_error {
            format!("Portal backend failed: {portal_err}; fallback backend failed: {err}")
        } else {
            err
        }
    })?;

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

fn parse_modifier_token(token: &str) -> Option<ShortcutKey> {
    match normalize_shortcut_token(token).as_str() {
        "ctrl" | "control" => Some(ShortcutKey::Control),
        "shift" => Some(ShortcutKey::Shift),
        "alt" | "option" => Some(ShortcutKey::Alt),
        "meta" | "super" | "win" | "windows" | "cmd" | "command" => Some(ShortcutKey::Meta),
        _ => None,
    }
}

fn parse_key_token(token: &str) -> Option<ShortcutKey> {
    if let Some(key) = parse_modifier_token(token) {
        return Some(key);
    }

    let normalized = normalize_shortcut_token(token);
    match normalized.as_str() {
        "enter" | "return" => Some(ShortcutKey::Return),
        "tab" => Some(ShortcutKey::Tab),
        "space" | "spacebar" => Some(ShortcutKey::Space),
        "esc" | "escape" => Some(ShortcutKey::Escape),
        "backspace" => Some(ShortcutKey::Backspace),
        "delete" | "del" => Some(ShortcutKey::Delete),
        "insert" | "ins" => Some(ShortcutKey::Insert),
        "home" => Some(ShortcutKey::Home),
        "end" => Some(ShortcutKey::End),
        "pageup" | "pgup" => Some(ShortcutKey::PageUp),
        "pagedown" | "pgdn" => Some(ShortcutKey::PageDown),
        "up" | "uparrow" => Some(ShortcutKey::UpArrow),
        "down" | "downarrow" => Some(ShortcutKey::DownArrow),
        "left" | "leftarrow" => Some(ShortcutKey::LeftArrow),
        "right" | "rightarrow" => Some(ShortcutKey::RightArrow),
        "capslock" => Some(ShortcutKey::CapsLock),
        "printscreen" | "prtsc" | "prtscr" | "printscr" => Some(ShortcutKey::PrintScreen),
        "pause" => Some(ShortcutKey::Pause),
        "scrolllock" => Some(ShortcutKey::ScrollLock),
        "numlock" => Some(ShortcutKey::NumLock),
        "menu" => Some(ShortcutKey::Menu),
        "plus" => Some(ShortcutKey::Character('+')),
        "minus" => Some(ShortcutKey::Character('-')),
        "comma" => Some(ShortcutKey::Character(',')),
        "period" | "dot" => Some(ShortcutKey::Character('.')),
        "slash" | "forwardslash" => Some(ShortcutKey::Character('/')),
        "backslash" => Some(ShortcutKey::Character('\\')),
        "semicolon" => Some(ShortcutKey::Character(';')),
        "quote" | "apostrophe" => Some(ShortcutKey::Character('\'')),
        "backtick" | "grave" => Some(ShortcutKey::Character('`')),
        "leftbracket" | "lbracket" => Some(ShortcutKey::Character('[')),
        "rightbracket" | "rbracket" => Some(ShortcutKey::Character(']')),
        _ => parse_function_key(&normalized).or_else(|| parse_single_character_key(token)),
    }
}

fn parse_function_key(token: &str) -> Option<ShortcutKey> {
    let number = token.strip_prefix('f')?.parse::<u8>().ok()?;
    (1..=35)
        .contains(&number)
        .then_some(ShortcutKey::Function(number))
}

fn parse_single_character_key(token: &str) -> Option<ShortcutKey> {
    let mut chars = token.trim().chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(ShortcutKey::Character(first))
}

fn normalize_shortcut_token(token: &str) -> String {
    token
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn should_try_portal_keyboard() -> bool {
    env::var_os("STREAMRS_DISABLE_PORTAL_KEYBOARD").is_none()
        && env::var_os("WAYLAND_DISPLAY").is_some()
}

fn send_keyboard_shortcut_via_portal(parsed: &ParsedShortcut) -> Result<(), String> {
    let cache = PORTAL_KEYBOARD_BACKEND.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .map_err(|_| "Portal keyboard backend mutex was poisoned".to_string())?;

    if guard.is_none() {
        *guard = Some(PortalKeyboardBackend::new()?);
    }

    let backend = guard
        .as_ref()
        .ok_or_else(|| "Portal keyboard backend cache was unexpectedly empty".to_string())?;

    if let Err(err) = backend.send_shortcut(parsed) {
        *guard = None;
        return Err(err);
    }

    Ok(())
}

fn send_keyboard_shortcut_with_enigo(
    shortcut: &str,
    parsed: &ParsedShortcut,
) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|err| format!("Failed to initialize keyboard input backend: {err}"))?;

        for key in &parsed.modifiers {
            enigo
                .key(
                    shortcut_key_to_enigo(*key).ok_or_else(|| {
                        format!(
                            "Keyboard shortcut key '{key:?}' is not supported by the fallback backend"
                        )
                    })?,
                    Press,
                )
                .map_err(|err| format!("Failed to press modifier for '{shortcut}': {err}"))?;
        }

        let trigger = shortcut_key_to_enigo(parsed.trigger).ok_or_else(|| {
            format!(
                "Keyboard shortcut key '{:?}' is not supported by the fallback backend",
                parsed.trigger
            )
        })?;
        let trigger_result = enigo
            .key(trigger, Click)
            .map_err(|err| format!("Failed to send keyboard shortcut '{shortcut}': {err}"));

        for key in parsed.modifiers.iter().rev() {
            if let Some(key) = shortcut_key_to_enigo(*key) {
                let _ = enigo.key(key, Release);
            }
        }

        trigger_result
    }))
    .map_err(|_| format!("Keyboard input backend panicked while sending shortcut '{shortcut}'"))?
}

fn shortcut_key_to_enigo(key: ShortcutKey) -> Option<Key> {
    match key {
        ShortcutKey::Control => Some(Key::Control),
        ShortcutKey::Shift => Some(Key::Shift),
        ShortcutKey::Alt => Some(Key::Alt),
        ShortcutKey::Meta => Some(Key::Meta),
        ShortcutKey::Return => Some(Key::Return),
        ShortcutKey::Tab => Some(Key::Tab),
        ShortcutKey::Space => Some(Key::Space),
        ShortcutKey::Escape => Some(Key::Escape),
        ShortcutKey::Backspace => Some(Key::Backspace),
        ShortcutKey::Delete => Some(Key::Delete),
        ShortcutKey::Insert => Some(Key::Insert),
        ShortcutKey::Home => Some(Key::Home),
        ShortcutKey::End => Some(Key::End),
        ShortcutKey::PageUp => Some(Key::PageUp),
        ShortcutKey::PageDown => Some(Key::PageDown),
        ShortcutKey::UpArrow => Some(Key::UpArrow),
        ShortcutKey::DownArrow => Some(Key::DownArrow),
        ShortcutKey::LeftArrow => Some(Key::LeftArrow),
        ShortcutKey::RightArrow => Some(Key::RightArrow),
        ShortcutKey::CapsLock => Some(Key::CapsLock),
        ShortcutKey::PrintScreen => Some(Key::PrintScr),
        ShortcutKey::Pause => Some(Key::Pause),
        ShortcutKey::ScrollLock => Some(Key::ScrollLock),
        ShortcutKey::NumLock => Some(Key::Numlock),
        ShortcutKey::Menu => Some(Key::LMenu),
        ShortcutKey::Function(1) => Some(Key::F1),
        ShortcutKey::Function(2) => Some(Key::F2),
        ShortcutKey::Function(3) => Some(Key::F3),
        ShortcutKey::Function(4) => Some(Key::F4),
        ShortcutKey::Function(5) => Some(Key::F5),
        ShortcutKey::Function(6) => Some(Key::F6),
        ShortcutKey::Function(7) => Some(Key::F7),
        ShortcutKey::Function(8) => Some(Key::F8),
        ShortcutKey::Function(9) => Some(Key::F9),
        ShortcutKey::Function(10) => Some(Key::F10),
        ShortcutKey::Function(11) => Some(Key::F11),
        ShortcutKey::Function(12) => Some(Key::F12),
        ShortcutKey::Function(13) => Some(Key::F13),
        ShortcutKey::Function(14) => Some(Key::F14),
        ShortcutKey::Function(15) => Some(Key::F15),
        ShortcutKey::Function(16) => Some(Key::F16),
        ShortcutKey::Function(17) => Some(Key::F17),
        ShortcutKey::Function(18) => Some(Key::F18),
        ShortcutKey::Function(19) => Some(Key::F19),
        ShortcutKey::Function(20) => Some(Key::F20),
        ShortcutKey::Function(21) => Some(Key::F21),
        ShortcutKey::Function(22) => Some(Key::F22),
        ShortcutKey::Function(23) => Some(Key::F23),
        ShortcutKey::Function(24) => Some(Key::F24),
        ShortcutKey::Function(25) => Some(Key::F25),
        ShortcutKey::Function(26) => Some(Key::F26),
        ShortcutKey::Function(27) => Some(Key::F27),
        ShortcutKey::Function(28) => Some(Key::F28),
        ShortcutKey::Function(29) => Some(Key::F29),
        ShortcutKey::Function(30) => Some(Key::F30),
        ShortcutKey::Function(31) => Some(Key::F31),
        ShortcutKey::Function(32) => Some(Key::F32),
        ShortcutKey::Function(33) => Some(Key::F33),
        ShortcutKey::Function(34) => Some(Key::F34),
        ShortcutKey::Function(35) => Some(Key::F35),
        ShortcutKey::Function(_) => None,
        ShortcutKey::Character(ch) => Some(Key::Unicode(ch)),
    }
}

fn shortcut_key_to_keysym(key: ShortcutKey) -> Option<i32> {
    let raw = match key {
        ShortcutKey::Control => keysym::Control_L,
        ShortcutKey::Shift => keysym::Shift_L,
        ShortcutKey::Alt => keysym::Alt_L,
        ShortcutKey::Meta => keysym::Super_L,
        ShortcutKey::Return => keysym::Return,
        ShortcutKey::Tab => keysym::Tab,
        ShortcutKey::Space => keysym::space,
        ShortcutKey::Escape => keysym::Escape,
        ShortcutKey::Backspace => keysym::BackSpace,
        ShortcutKey::Delete => keysym::Delete,
        ShortcutKey::Insert => keysym::Insert,
        ShortcutKey::Home => keysym::Home,
        ShortcutKey::End => keysym::End,
        ShortcutKey::PageUp => keysym::Page_Up,
        ShortcutKey::PageDown => keysym::Page_Down,
        ShortcutKey::UpArrow => keysym::Up,
        ShortcutKey::DownArrow => keysym::Down,
        ShortcutKey::LeftArrow => keysym::Left,
        ShortcutKey::RightArrow => keysym::Right,
        ShortcutKey::CapsLock => keysym::Caps_Lock,
        ShortcutKey::PrintScreen => keysym::Print,
        ShortcutKey::Pause => keysym::Pause,
        ShortcutKey::ScrollLock => keysym::Scroll_Lock,
        ShortcutKey::NumLock => keysym::Num_Lock,
        ShortcutKey::Menu => keysym::Menu,
        ShortcutKey::Function(number) => match number {
            1 => keysym::F1,
            2 => keysym::F2,
            3 => keysym::F3,
            4 => keysym::F4,
            5 => keysym::F5,
            6 => keysym::F6,
            7 => keysym::F7,
            8 => keysym::F8,
            9 => keysym::F9,
            10 => keysym::F10,
            11 => keysym::F11,
            12 => keysym::F12,
            13 => keysym::F13,
            14 => keysym::F14,
            15 => keysym::F15,
            16 => keysym::F16,
            17 => keysym::F17,
            18 => keysym::F18,
            19 => keysym::F19,
            20 => keysym::F20,
            21 => keysym::F21,
            22 => keysym::F22,
            23 => keysym::F23,
            24 => keysym::F24,
            25 => keysym::F25,
            26 => keysym::F26,
            27 => keysym::F27,
            28 => keysym::F28,
            29 => keysym::F29,
            30 => keysym::F30,
            31 => keysym::F31,
            32 => keysym::F32,
            33 => keysym::F33,
            34 => keysym::F34,
            35 => keysym::F35,
            _ => return None,
        },
        ShortcutKey::Character(ch) => match ch {
            ' ' => keysym::space,
            _ => xkeysym::Keysym::from_char(ch).raw(),
        },
    };

    i32::try_from(raw).ok()
}

fn next_portal_token(prefix: &str) -> String {
    let suffix = PORTAL_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{suffix}")
}

fn streamrs_state_path() -> PathBuf {
    crate::paths::streamrs_state_path()
}

fn load_streamrs_state() -> Result<StreamrsState, String> {
    let path = streamrs_state_path();
    match crate::config::toml::load_from_file(&path) {
        Ok(state) => Ok(state),
        Err(_) if matches!(fs::metadata(&path), Err(metadata_err) if metadata_err.kind() == ErrorKind::NotFound) => {
            Ok(StreamrsState::default())
        }
        Err(err) => Err(err),
    }
}

fn save_streamrs_state(state: &StreamrsState) -> Result<(), String> {
    let path = streamrs_state_path();
    crate::config::toml::save_to_file_pretty(&path, state)?;

    #[cfg(unix)]
    fs::set_permissions(&path, Permissions::from_mode(0o600)).map_err(|err| {
        format!(
            "Failed to secure streamrs state file '{}': {err}",
            path.display()
        )
    })?;

    Ok(())
}

fn load_portal_restore_token() -> Result<Option<String>, String> {
    Ok(load_streamrs_state()?
        .portal_remote_desktop_restore_token
        .and_then(|token| {
            let trimmed = token.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }))
}

fn save_portal_restore_token(token: &str) -> Result<(), String> {
    let mut state = load_streamrs_state()?;
    state.portal_remote_desktop_restore_token = Some(token.trim().to_string());
    save_streamrs_state(&state)
}

fn clear_portal_restore_token() -> Result<(), String> {
    let path = streamrs_state_path();
    let mut state = load_streamrs_state()?;
    state.portal_remote_desktop_restore_token = None;

    if state == StreamrsState::default() {
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "Failed to remove streamrs state file '{}': {err}",
                path.display()
            )),
        }
    } else {
        save_streamrs_state(&state)
    }
}

fn portal_request_path(connection: &gio::DBusConnection, token: &str) -> Result<String, String> {
    let sender = connection
        .unique_name()
        .ok_or_else(|| "Session bus connection has no unique name".to_string())?;
    Ok(format!(
        "{PORTAL_DESKTOP_PATH}/request/{}/{}",
        sanitize_dbus_path_component(sender.as_str()),
        token
    ))
}

fn sanitize_dbus_path_component(value: &str) -> String {
    let value = value.strip_prefix(':').unwrap_or(value);
    value
        .chars()
        .map(|ch| match ch {
            '.' => '_',
            _ if ch.is_ascii_alphanumeric() || ch == '_' => ch,
            _ => '_',
        })
        .collect()
}

fn portal_options(entries: &[(&str, glib::Variant)]) -> HashMap<String, glib::Variant> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

fn wait_for_portal_response<F>(
    connection: &gio::DBusConnection,
    request_path: &str,
    invoke: F,
) -> Result<HashMap<String, glib::Variant>, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let request_path = request_path.to_string();
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let main_loop = glib::MainLoop::new(Some(&context), false);
            let response = Arc::new(Mutex::new(
                None::<Result<HashMap<String, glib::Variant>, String>>,
            ));

            let response_for_signal = Arc::clone(&response);
            let loop_for_signal = main_loop.clone();
            let subscription = connection.subscribe_to_signal(
                Some(PORTAL_BUS_NAME),
                Some(PORTAL_REQUEST_INTERFACE),
                Some("Response"),
                Some(&request_path),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let result = <(u32, HashMap<String, glib::Variant>)>::from_variant(
                        signal.parameters,
                    )
                    .ok_or_else(|| {
                        "Portal returned an invalid Response payload for keyboard setup".to_string()
                    })
                    .and_then(|(code, results)| match code {
                        PORTAL_RESPONSE_SUCCESS => Ok(results),
                        PORTAL_RESPONSE_CANCELLED => {
                            Err("Portal keyboard permission request was cancelled".to_string())
                        }
                        other => Err(format!(
                            "Portal keyboard permission request failed with response code {other}"
                        )),
                    });

                    if let Ok(mut slot) = response_for_signal.lock() {
                        *slot = Some(result);
                    }
                    loop_for_signal.quit();
                },
            );

            let response_for_timeout = Arc::clone(&response);
            let loop_for_timeout = main_loop.clone();
            let request_path_for_timeout = request_path.clone();
            let timeout_id = glib::timeout_add_local_once(PORTAL_REQUEST_TIMEOUT, move || {
                if let Ok(mut slot) = response_for_timeout.lock()
                    && slot.is_none()
                {
                    *slot = Some(Err(format!(
                        "Timed out waiting for portal response on '{request_path_for_timeout}'"
                    )));
                }
                loop_for_timeout.quit();
            });

            invoke()?;
            main_loop.run();
            timeout_id.remove();
            drop(subscription);

            response
                .lock()
                .map_err(|_| "Portal response mutex was poisoned".to_string())?
                .take()
                .unwrap_or_else(|| {
                    Err(format!(
                        "Portal request '{request_path}' completed without a response"
                    ))
                })
        })
        .map_err(|err| format!("Failed to acquire GLib main context for portal call: {err}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Mutex, OnceLock};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_xdg_state_home(name: &str, run: impl FnOnce()) {
        let _guard = env_lock().lock().expect("env lock should be available");
        let id = TEST_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-process-tests-{name}-{id}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("test temp state dir should be creatable");

        let previous = std::env::var_os("XDG_STATE_HOME");
        // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", &dir);
        }

        run();

        if let Some(value) = previous {
            // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
            unsafe {
                std::env::set_var("XDG_STATE_HOME", value);
            }
        } else {
            // SAFETY: Tests hold a process-wide mutex so env mutation is serialized.
            unsafe {
                std::env::remove_var("XDG_STATE_HOME");
            }
        }

        let _ = fs::remove_dir_all(dir);
    }

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
                modifiers: vec![ShortcutKey::Control, ShortcutKey::Shift],
                trigger: ShortcutKey::Character('T'),
            }
        );
        assert_eq!(
            parse_keyboard_shortcut("Meta+Return").expect("shortcut should parse"),
            ParsedShortcut {
                modifiers: vec![ShortcutKey::Meta],
                trigger: ShortcutKey::Return,
            }
        );
        assert_eq!(
            parse_keyboard_shortcut("Ctrl+Alt+Delete").expect("shortcut should parse"),
            ParsedShortcut {
                modifiers: vec![ShortcutKey::Control, ShortcutKey::Alt],
                trigger: ShortcutKey::Delete,
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

    #[test]
    fn dbus_sender_name_is_sanitized_for_portal_request_paths() {
        assert_eq!(sanitize_dbus_path_component(":1.42"), "1_42");
        assert_eq!(
            sanitize_dbus_path_component("org.example.App"),
            "org_example_App"
        );
    }

    #[test]
    fn portal_restore_token_io_round_trips() {
        with_temp_xdg_state_home("portal-token-roundtrip", || {
            save_portal_restore_token("token-123").expect("token should save");
            assert_eq!(
                load_portal_restore_token().expect("token should load"),
                Some("token-123".to_string())
            );

            #[cfg(unix)]
            {
                let metadata =
                    fs::metadata(streamrs_state_path()).expect("token file metadata should exist");
                assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            }

            clear_portal_restore_token().expect("token should clear");
            assert_eq!(
                load_portal_restore_token().expect("missing token should be tolerated"),
                None
            );
        });
    }
}
