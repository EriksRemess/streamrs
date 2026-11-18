mod core;
mod ui;

pub(crate) use core::*;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use ui::build_ui;

fn gui_lock_path() -> PathBuf {
    gtk::glib::user_runtime_dir().join("streamrs-gui.flock")
}

fn try_acquire_gui_lock(path: &Path) -> Result<File, String> {
    // Keep one stable inode. Closing the file releases the lock, including after a crash.
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|err| {
            trf(
                "Failed to create GUI lock '{path}': {err}",
                &[
                    ("path", path.display().to_string()),
                    ("err", err.to_string()),
                ],
            )
        })?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(trf(
            "Another streamrs-gui instance is already running (lock '{path}').",
            &[("path", path.display().to_string())],
        )),
        Err(TryLockError::Error(err)) => Err(trf(
            "Failed to create GUI lock '{path}': {err}",
            &[
                ("path", path.display().to_string()),
                ("err", err.to_string()),
            ],
        )),
    }
}

pub(crate) fn run() {
    init_i18n();

    let lock_path = gui_lock_path();
    let _gui_lock = match try_acquire_gui_lock(&lock_path) {
        Ok(lock) => lock,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    let _ = adw::init();
    gtk::glib::set_application_name("streamrs");

    let app = Application::builder()
        .application_id("lv.apps.streamrs")
        .build();

    app.connect_activate(build_ui);

    let _ = app.run_with_args(&env::args().collect::<Vec<_>>());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandoned_empty_lock_recovers_and_excludes_a_second_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gui.flock");
        std::fs::write(&path, "").unwrap();
        let owner = try_acquire_gui_lock(&path).unwrap();
        assert!(try_acquire_gui_lock(&path).is_err());
        drop(owner);
        drop(try_acquire_gui_lock(&path).unwrap());
    }
}
