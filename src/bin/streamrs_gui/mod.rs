include!("core.rs");
include!("ui.rs");

pub fn run() {
    let _ = adw::init();
    gtk::glib::set_application_name("streamrs");

    let app = Application::builder()
        .application_id("lv.apps.streamrs")
        .build();

    app.connect_activate(build_ui);

    let _ = app.run_with_args(&env::args().collect::<Vec<_>>());
}
