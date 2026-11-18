use std::{env, io::ErrorKind, path::PathBuf, process::Command};

fn main() {
    println!("cargo::rerun-if-changed=scripts/build-translations.sh");
    println!("cargo::rerun-if-changed=po/LINGUAS");
    for lang in include_str!("po/LINGUAS").split_whitespace() {
        println!("cargo::rerun-if-changed=po/{lang}.po");
    }
    if let Err(err) = Command::new("msgfmt").arg("--version").output() {
        if err.kind() == ErrorKind::NotFound {
            println!("cargo::warning=Install gettext to build the bundled translations");
            return;
        }
        panic!("Could not start msgfmt: {err}");
    }
    let locale = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("locale");
    let status = Command::new("bash")
        .arg("scripts/build-translations.sh")
        .arg(locale)
        .status()
        .expect("Could not compile translation catalogs");
    assert!(status.success(), "Failed to compile translation catalogs");
}
