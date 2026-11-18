use super::*;
use streamrs::streamdeck::info::DeviceInfo;

pub(super) fn register(window: &ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    let action = gtk::gio::SimpleAction::new("device-info", None);
    let parent = window.downgrade();
    let state = Rc::clone(state);
    action.connect_activate(move |_, _| {
        let Some(window) = parent.upgrade() else {
            return;
        };
        let state = state.borrow();
        let config = &state.config;
        let (vendor, product, usage, page) = (
            config.vendor_id,
            config.product_id,
            config.usage,
            config.usage_page,
        );
        let dialog = adw::AlertDialog::new(
            Some(&tr("Device information")),
            Some(&tr("Reading device information…")),
        );
        dialog.add_response("close", &tr("Close"));
        dialog.set_close_response("close");
        dialog.present(Some(&window));
        let dialog = dialog.downgrade();
        gtk::glib::spawn_future_local(async move {
            let result =
                gtk::gio::spawn_blocking(move || DeviceInfo::read(vendor, product, usage, page))
                    .await;
            let Some(dialog) = dialog.upgrade() else {
                return;
            };
            match result {
                Ok(Ok(info)) => {
                    let grid = gtk::Grid::builder()
                        .row_spacing(6)
                        .column_spacing(16)
                        .halign(Align::Center)
                        .build();
                    for (i, (name, value)) in info.rows(tr).into_iter().enumerate() {
                        let title = Label::new(Some(&name));
                        title.set_xalign(0.0);
                        title.add_css_class("dim-label");
                        let value = Label::new(Some(&value));
                        value.set_xalign(0.0);
                        value.set_selectable(true);
                        grid.attach(&title, 0, i as i32, 1, 1);
                        grid.attach(&value, 1, i as i32, 1, 1);
                    }
                    dialog.set_body("");
                    dialog.set_extra_child(Some(&grid));
                }
                Ok(Err(err)) => dialog.set_body(&err),
                Err(_) => dialog.set_body(&tr("Could not read device information.")),
            }
        });
    });
    window.add_action(&action);
}
