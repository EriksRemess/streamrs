use super::*;
use streamrs::streamdeck::info::{DeviceInfo, SleepTimer, open_info_device};

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
                    if let Some(timer) = info.sleep_timer {
                        dialog.add_response("sleep", &tr("Change sleep timer…"));
                        let device = Cell::new(Some(device));
                        dialog.connect_response(Some("sleep"), move |_, _| {
                            if let (Some(window), Some(device)) = (window.upgrade(), device.take())
                            {
                                gtk::glib::spawn_future_local(edit_sleep_timer(
                                    window, device, timer,
                                ));
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

async fn edit_sleep_timer(
    window: ApplicationWindow,
    device: hidapi::HidDevice,
    current: SleepTimer,
) {
    let dialog = adw::AlertDialog::new(
        Some(&tr("Sleep after inactivity")),
        Some(&tr(
            "Enter the number of minutes before the deck sleeps. Use 0 for never.",
        )),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("apply", &tr("Apply"));
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("apply"));
    dialog.set_response_appearance("apply", ResponseAppearance::Suggested);
    dialog.set_response_enabled("apply", false);

    let entry = gtk::Entry::builder()
        .input_purpose(gtk::InputPurpose::Digits)
        .placeholder_text(tr("Minutes"))
        .activates_default(true)
        .build();
    if let Some(minutes) = current.whole_minutes() {
        entry.set_text(&minutes.to_string());
    }
    let weak = dialog.downgrade();
    entry.connect_changed(move |entry| {
        if let Some(dialog) = weak.upgrade() {
            let value = entry.text().parse().ok().and_then(SleepTimer::from_minutes);
            dialog.set_response_enabled("apply", value.is_some_and(|timer| timer != current));
        }
    });
    dialog.set_extra_child(Some(&entry));
    if dialog.choose_future(Some(&window)).await != "apply" {
        return;
    }
    let Some(timer) = entry.text().parse().ok().and_then(SleepTimer::from_minutes) else {
        return;
    };
    let result = gtk::gio::spawn_blocking(move || timer.apply(&device)).await;
    let error = match result {
        Ok(Ok(())) => return,
        Ok(Err(err)) => err,
        Err(_) => tr("Could not change the sleep timer."),
    };
    let dialog = adw::AlertDialog::new(Some(&tr("Sleep after inactivity")), Some(&error));
    dialog.add_response("close", &tr("Close"));
    dialog.present(Some(&window));
}
