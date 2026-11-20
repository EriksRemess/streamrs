use super::*;

pub(crate) fn load_config(path: &Path) -> Result<Config, String> {
    let profile = profile_from_config_path(path);
    let mut config = streamrs_profile::load_config_for_profile(path, &profile)?;
    normalize_config(&mut config);
    Ok(config)
}

pub(crate) fn update_config(path: &Path, edit: impl FnOnce(&mut Config)) -> Result<Config, String> {
    streamrs_profile::try_update_config(path, |config| {
        normalize_config(config);
        edit(config);
    })
}

pub(crate) use streamrs_profile::create_if_missing as create_config_if_missing;

#[cfg(target_os = "linux")]
fn is_daemon_command(cmdline: &[u8]) -> bool {
    let mut args = cmdline
        .strip_suffix(&[0])
        .unwrap_or(cmdline)
        .split(|&b| b == 0);
    if args
        .next()
        .and_then(|program| program.rsplit(|&b| b == b'/').next())
        != Some(b"streamrs")
    {
        return false;
    }
    while let Some(arg) = args.next() {
        match arg {
            b"--debug" => {}
            b"--config" | b"--profile" if args.next().is_some() => {}
            _ => return false,
        }
    }
    true
}

#[cfg(target_os = "linux")]
fn catches_signal(status: &str, signal: libc::c_int) -> bool {
    status
        .lines()
        .find_map(|line| line.strip_prefix("SigCgt:"))
        .and_then(|mask| u64::from_str_radix(mask.trim(), 16).ok())
        .is_some_and(|mask| mask & (1 << (signal - 1)) != 0)
}

#[cfg(target_os = "linux")]
fn owns_deck(process: &Path, deck: &Path) -> bool {
    let Ok(fds) = fs::read_dir(process.join("fd")) else {
        return false;
    };
    fds.flatten().any(|fd| {
        fs::read_link(fd.path()).is_ok_and(|path| path == deck)
            && fs::read_to_string(process.join("fdinfo").join(fd.file_name())).is_ok_and(|info| {
                info.lines().any(|line| {
                    line.starts_with("lock:") && line.split_whitespace().nth(2) == Some("OFDLCK")
                })
            })
    })
}

#[cfg(target_os = "linux")]
fn matching_daemons(signal: Option<libc::c_int>, deck: Option<&Path>) -> std::io::Result<bool> {
    use rustix::process::{Pid, PidfdFlags, Signal, geteuid, pidfd_open, pidfd_send_signal};
    use std::os::unix::fs::MetadataExt;

    let mut found = false;
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse().ok())
            .and_then(Pid::from_raw)
        else {
            continue;
        };
        let path = entry.path();
        if !entry
            .metadata()
            .is_ok_and(|m| m.uid() == geteuid().as_raw())
            || fs::read(path.join("comm")).as_deref().ok() != Some(b"streamrs\n")
        {
            continue;
        }
        // Pin the process before inspecting its arguments so PID reuse cannot redirect a signal.
        let fd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::SRCH) => continue,
            Err(err) => return Err(err.into()),
        };
        if !fs::read(path.join("cmdline")).is_ok_and(|args| is_daemon_command(&args))
            || deck.is_some_and(|deck| !owns_deck(&path, deck))
        {
            continue;
        }
        if let Some(mut signal) = signal {
            // A starting daemon still has the default SIGHUP action. Its first config read
            // follows handler installation, so it already sees the saved changes.
            if signal == libc::SIGHUP {
                let status = match fs::read_to_string(path.join("status")) {
                    Ok(status) => status,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(err) => return Err(err),
                };
                if deck.is_some() && catches_signal(&status, libc::SIGUSR1) {
                    signal = libc::SIGUSR1;
                } else if !catches_signal(&status, libc::SIGHUP) {
                    found |= deck.is_none();
                    continue;
                }
            }
            match pidfd_send_signal(&fd, Signal::from_named_raw(signal).unwrap()) {
                Ok(()) => {}
                Err(rustix::io::Errno::SRCH) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        found = true;
    }
    Ok(found)
}

#[cfg(not(target_os = "linux"))]
fn matching_daemons(_: Option<libc::c_int>, _: Option<&Path>) -> std::io::Result<bool> {
    Ok(false)
}

fn service_command(args: &[&str]) -> Result<(), String> {
    let status = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .arg("streamrs.service")
        .status()
        .map_err(|err| trf("systemctl failed: {err}", &[("err", err.to_string())]))?;
    if status.success() {
        Ok(())
    } else {
        Err(trf(
            "systemctl exit status {status}",
            &[("status", status.to_string())],
        ))
    }
}

pub(crate) fn signal_daemon_reload() -> Result<(), String> {
    let service_reload = || service_command(&["kill", "-s", "HUP", "--kill-whom=main"]);
    match matching_daemons(Some(libc::SIGHUP), None) {
        Ok(true) => Ok(()),
        Ok(false) => service_reload(),
        Err(err) => service_reload().map_err(|service_err| format!("{service_err} ({err})")),
    }
    .map_err(|err| trf("Failed to signal streamrs daemon: {err}", &[("err", err)]))
}

pub(crate) fn restore_color_preview(
    display: streamrs::streamdeck::DeckDisplay,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;

        if !display.try_claim()? {
            let info = display
                .hid()
                .get_device_info()
                .map_err(|err| err.to_string())?;
            let deck = Path::new(std::ffi::OsStr::from_bytes(info.path().to_bytes()));
            if matching_daemons(Some(libc::SIGHUP), Some(deck)).map_err(|err| err.to_string())? {
                return Ok(());
            }
            // The daemon may have stopped while its descriptors were being inspected.
            if !display.try_claim()? {
                return Err(tr("Could not ask the daemon to redraw this deck."));
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    if daemon_running() {
        return signal_daemon_reload();
    }
    display.show_standby_image()
}

pub(crate) fn daemon_running() -> bool {
    service_command(&["is-active", "--quiet"]).is_ok()
        || matching_daemons(None, None).unwrap_or(false)
}

pub(crate) fn set_daemon_running(should_run: bool) -> Result<(), String> {
    if should_run {
        return service_command(&["start"])
            .map_err(|err| trf("Failed to start streamrs daemon: {err}", &[("err", err)]));
    }
    let stopped = service_command(&["stop"]);
    match matching_daemons(Some(libc::SIGTERM), None) {
        Ok(true) => Ok(()),
        Ok(false) => stopped,
        Err(err) => Err(err.to_string()),
    }
    .map_err(|err| trf("Failed to stop streamrs daemon: {err}", &[("err", err)]))
}

pub(crate) fn restart_daemon() -> Result<(), String> {
    if daemon_running() {
        set_daemon_running(false)?;
    }
    set_daemon_running(true)
}

pub(crate) fn profile_slug_from_input(raw: &str) -> Option<String> {
    profile_slug_from_input_generic(raw)
}

pub(crate) fn profile_display_name(profile: &str) -> String {
    profile_display_name_generic(profile)
}

pub(crate) fn discover_profiles() -> Vec<String> {
    discover_profiles_generic()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[cfg(target_os = "linux")]
    #[test]
    fn reload_waits_for_the_daemons_signal_handler() {
        assert!(!catches_signal(
            "SigIgn:\t0000000000000001\nSigCgt:\t0000000000004000\n",
            libc::SIGHUP
        ));
        assert!(catches_signal("SigCgt:\t0000000000004001\n", libc::SIGHUP));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn redraw_selects_the_daemon_with_a_claim_on_the_requested_deck() {
        use std::os::unix::fs::symlink;

        let proc = tempfile::tempdir().unwrap();
        let path = proc.path();
        fs::create_dir(path.join("fd")).unwrap();
        fs::create_dir(path.join("fdinfo")).unwrap();
        let deck = Path::new("/dev/hidraw3");
        symlink(deck, path.join("fd/5")).unwrap();
        fs::write(path.join("fdinfo/5"), "pos:\t0\n").unwrap();
        assert!(!owns_deck(path, deck));
        fs::write(
            path.join("fdinfo/5"),
            "lock:\t1: OFDLCK ADVISORY  WRITE -1 00:07:12 0 0\n",
        )
        .unwrap();
        assert!(owns_deck(path, deck));
        assert!(!owns_deck(path, Path::new("/dev/hidraw4")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gui_signals_only_daemon_invocations() {
        for args in [
            &b"/usr/bin/streamrs\0"[..],
            b"streamrs\0--debug\0--config\0my profile.toml\0",
            b"streamrs\0--profile\0desk\0",
        ] {
            assert!(is_daemon_command(args));
        }
        for args in [
            &b""[..],
            b"other-process\0",
            b"streamrs\0--init\0--force-images\0",
            b"streamrs\0--config\0my profile.toml\0--standby-image\0logo.png\0",
            b"streamrs\0--info\0",
            b"streamrs\0--sleep-after\0never\0",
            b"streamrs\0--brightness\x0050\0",
        ] {
            assert!(!is_daemon_command(args));
        }
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("streamrs-gui-config-io-tests-{name}-{id}"));
        fs::create_dir_all(&dir).expect("test directory should be creatable");
        dir
    }

    #[test]
    fn wallpaper_updates_save_the_page_size_shown_in_the_editor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("desk.toml");
        fs::write(
            &path,
            "keys_per_page = 99\nbrightness = 35\n[[keys]]\nicon = 'blank.png'\n",
        )
        .unwrap();
        let displayed = load_config(&path).unwrap().keys_per_page;
        update_config(&path, |config| {
            config.page_wallpapers.insert("1".into(), "wall.png".into());
        })
        .unwrap();
        let saved: Config = streamrs::config::toml::load_from_file(&path).unwrap();
        assert_eq!(saved.keys_per_page, displayed);
        assert_eq!(saved.brightness, 35);
        assert_eq!(saved.page_wallpaper(0), Some(Path::new("wall.png")));
    }

    #[test]
    fn load_config_missing_profile_uses_blank_template_not_fallback_default() {
        let dir = test_temp_dir("missing-profile");
        let path = dir.join("new_profile.toml");
        let config = load_config(&path).expect("missing profile config should load as template");
        assert_eq!(config.keys.len(), KEY_COUNT);
        assert_eq!(config.keys[0].icon, "blank.png");
        assert!(
            config.keys[0].action.is_none(),
            "new profile template should not inherit launcher actions"
        );
    }

    #[test]
    fn load_config_reads_exact_profile_file_when_present() {
        let dir = test_temp_dir("existing-profile");
        let path = dir.join("test_profile.toml");
        fs::write(
            &path,
            r#"
            [[keys]]
            icon = "custom.png"
            "#,
        )
        .expect("fixture config should be written");

        let config = load_config(&path).expect("existing profile config should load");
        assert_eq!(config.keys[0].icon, "custom.png");
    }

    #[test]
    fn load_config_blank_profile_remains_empty() {
        let dir = test_temp_dir("blank-profile");
        let path = dir.join("blank.toml");
        let config = load_config(&path).expect("blank profile config should load");
        assert!(
            config.keys.is_empty(),
            "blank profile must remain empty and show black deck"
        );
    }
}
