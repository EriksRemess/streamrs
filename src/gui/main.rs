mod core;
mod ui;

pub(crate) use core::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use ui::build_ui;

struct GuiInstanceLock {
    path: PathBuf,
}

impl Drop for GuiInstanceLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn gui_lock_path() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("streamrs-gui.lock")
    } else {
        env::temp_dir().join("streamrs-gui.lock")
    }
}

fn pid_is_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn try_acquire_gui_lock(path: &Path) -> Result<GuiInstanceLock, String> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            let pid = std::process::id();
            file.write_all(pid.to_string().as_bytes())
                .map_err(|err| format!("Failed to write GUI lock '{}': {err}", path.display()))?;
            Ok(GuiInstanceLock {
                path: path.to_path_buf(),
            })
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let stale_pid = fs::read_to_string(path)
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .filter(|pid| !pid_is_running(*pid));
            if stale_pid.is_some() {
                let _ = fs::remove_file(path);
                return try_acquire_gui_lock(path);
            }
            Err(format!(
                "Another streamrs-gui instance is already running (lock '{}').",
                path.display()
            ))
        }
        Err(err) => Err(format!(
            "Failed to create GUI lock '{}': {err}",
            path.display()
        )),
    }
}

pub(crate) fn run() {
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
