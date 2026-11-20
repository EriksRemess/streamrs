use super::*;
use streamrs::streamdeck::{
    firmware_image,
    info::{DeviceInfo, open_info_device},
};

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
        let window = window.downgrade();
        gtk::glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || {
                let device = open_info_device(vendor, product, usage, page)?;
                let info = DeviceInfo::from_device(&device, vendor, product);
                Ok::<_, String>((info, device))
            })
            .await;
            let Some(dialog) = dialog.upgrade() else {
                return;
            };
            match result {
                Ok(Ok((info, device))) => {
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
                    if firmware_image::supports_device(vendor, product) {
                        dialog.add_response("compare", &tr("Compare firmware image…"));
                        let device = Cell::new(Some(device));
                        dialog.connect_response(Some("compare"), move |_, _| {
                            if let (Some(window), Some(device)) = (window.upgrade(), device.take())
                            {
                                gtk::glib::spawn_future_local(compare_firmware(window, device));
                            }
                        });
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

async fn compare_firmware(window: ApplicationWindow, device: hidapi::HidDevice) {
    let title = tr("Compare firmware image");
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&tr("Firmware images")));
    filter.add_suffix("bin");
    let result = match choose_local_file(&window, &title, &filter).await {
        Ok(None) => return,
        Ok(Some(path)) => gtk::gio::spawn_blocking(move || firmware_image::compare(&device, &path))
            .await
            .unwrap_or_else(|_| Err(tr("Device command failed.")))
            .map(|comparison| comparison.summary()),
        Err(err) => Err(err),
    };
    let message = result.unwrap_or_else(|err| err);
    let dialog = adw::AlertDialog::new(Some(&title), Some(&message));
    dialog.add_response("close", &tr("Close"));
    dialog.present(Some(&window));
}
