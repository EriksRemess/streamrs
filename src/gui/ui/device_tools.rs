use super::*;
use streamrs::streamdeck::{
    DeckDisplay, DisplayFill,
    info::{SleepTimer, open_info_device},
    set_standby_image, supports_standby_upload,
};

#[derive(Clone, Copy)]
enum Tool {
    DisplayColor,
    StandbyImage,
    SleepTimer,
}

pub(super) fn register(window: &ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    for (name, title, tool) in [
        (
            "display-color",
            tr("Test display color"),
            Tool::DisplayColor,
        ),
        ("standby-image", tr("Set standby image"), Tool::StandbyImage),
        (
            "sleep-timer",
            tr("Sleep after inactivity"),
            Tool::SleepTimer,
        ),
    ] {
        let action = gtk::gio::SimpleAction::new(name, None);
        let parent = window.downgrade();
        let state = Rc::clone(state);
        action.connect_activate(move |action, _| {
            let Some(window) = parent.upgrade() else {
                return;
            };
            let ids = {
                let state = state.borrow();
                let config = &state.config;
                (
                    config.vendor_id,
                    config.product_id,
                    config.usage,
                    config.usage_page,
                )
            };
            let title = title.clone();
            let action = action.clone();
            action.set_enabled(false);
            gtk::glib::spawn_future_local(async move {
                let result = tool.run(&window, &title, ids).await;
                action.set_enabled(true);
                match result {
                    Ok(None) => {}
                    Ok(Some(message)) | Err(message) => {
                        let dialog = adw::AlertDialog::new(Some(&title), Some(&message));
                        dialog.add_response("close", &tr("Close"));
                        dialog.present(Some(&window));
                    }
                }
            });
        });
        window.add_action(&action);
    }
}

impl Tool {
    async fn run(
        self,
        window: &ApplicationWindow,
        title: &str,
        ids: (u16, u16, u16, u16),
    ) -> Result<Option<String>, String> {
        let (vendor, product, usage, page) = ids;
        let open = move || open_info_device(vendor, product, usage, page);
        match self {
            Tool::DisplayColor => {
                preview_color(window, title, open).await?;
            }
            Tool::StandbyImage => {
                if !supports_standby_upload(vendor, product) {
                    return Err(tr(
                        "Custom standby images require a standard Stream Deck MK.2 (0fd9:0080).",
                    ));
                }
                let device = gtk::gio::spawn_blocking(open)
                    .await
                    .map_err(|_| tr("Device command failed."))??;
                if let Some(path) = choose_image(window, title).await? {
                    gtk::gio::spawn_blocking(move || set_standby_image(&device, &path))
                        .await
                        .map_err(|_| tr("Could not upload the standby image."))??;
                    return Ok(Some(tr("Standby image sent to the deck.")));
                }
            }
            Tool::SleepTimer => {
                let (device, current) = gtk::gio::spawn_blocking(move || {
                    let device = open()?;
                    let current = SleepTimer::read(&device)
                        .ok_or_else(|| tr("Could not read the sleep timer."))?;
                    Ok::<_, String>((device, current))
                })
                .await
                .map_err(|_| tr("Could not read the sleep timer."))??;
                if let Some(timer) = choose_sleep_timer(window, title, current).await {
                    gtk::gio::spawn_blocking(move || timer.apply(&device))
                        .await
                        .map_err(|_| tr("Could not change the sleep timer."))??;
                }
            }
        }
        Ok(None)
    }
}

async fn preview_color(
    window: &ApplicationWindow,
    title: &str,
    open: impl Fn() -> Result<hidapi::HidDevice, String> + Send + Copy + 'static,
) -> Result<(), String> {
    let _hold = window.application().map(|app| app.hold());
    let hint = tr("Apply previews the color. Cancel restores the display.");
    let dialog = adw::AlertDialog::new(Some(title), Some(&hint));
    let options: Vec<_> = std::iter::once(tr("Entire display"))
        .chain((1..=KEY_COUNT).map(|n| trf("Button {index}", &[("index", n.to_string())])))
        .collect();
    let options: Vec<_> = options.iter().map(String::as_str).collect();
    let target = gtk::DropDown::from_strings(&options);
    let color = gtk::ColorButton::with_rgba(&gtk::gdk::RGBA::WHITE);
    color.set_use_alpha(false);
    color.set_title(&tr("Display color"));
    color.set_tooltip_text(Some(&tr("Display color")));
    let controls = GtkBox::new(Orientation::Horizontal, 8);
    controls.set_halign(Align::Center);
    controls.append(&target);
    controls.append(&color);
    let apply = Button::with_label(&tr("Apply"));
    apply.add_css_class("suggested-action");
    let cancel = Button::with_label(&tr("Cancel"));
    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.set_homogeneous(true);
    buttons.append(&cancel);
    buttons.append(&apply);
    let contents = GtkBox::new(Orientation::Vertical, UI_SPACING);
    contents.append(&controls);
    contents.append(&buttons);
    dialog.set_extra_child(Some(&contents));

    let (send, mut requests) = futures_channel::mpsc::unbounded();
    apply.connect_clicked({
        let send = send.clone();
        move |button| {
            button.set_sensitive(false);
            let rgba = color.rgba();
            let rgb = [rgba.red(), rgba.green(), rgba.blue()].map(|v| (v * 255.0).round() as u8);
            let fill = match target.selected() {
                0 => Ok(DisplayFill::lcd(rgb)),
                number => u8::try_from(number)
                    .map_err(|_| tr("Select a key."))
                    .and_then(|n| DisplayFill::key(n, rgb)),
            };
            let _ = send.unbounded_send(fill);
        }
    });
    let weak_dialog = dialog.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(dialog) = weak_dialog.upgrade() {
            dialog.close();
        }
    });
    dialog.connect_closed({
        let send = send.clone();
        move |_| send.close_channel()
    });
    let unmap = window.connect_unmap(move |_| send.close_channel());
    dialog.present(Some(window));

    let mut display: Option<DeckDisplay> = None;
    while let Ok(request) = requests.recv().await {
        let (device, result) = gtk::gio::spawn_blocking(move || {
            let result = request.and_then(|fill| {
                if let Some(active) = &display {
                    return active.apply_fill(fill);
                }
                let active = DeckDisplay::new(open()?)?;
                active.apply_fill(fill)?;
                display = Some(active);
                Ok(())
            });
            (display, result)
        })
        .await
        .map_err(|_| tr("Device command failed."))?;
        dialog.set_body(result.as_ref().err().unwrap_or(&hint));
        display = device;
        apply.set_sensitive(true);
    }
    window.disconnect(unmap);
    if let Some(display) = display {
        gtk::gio::spawn_blocking(move || restore_color_preview(display))
            .await
            .map_err(|_| tr("Device command failed."))??;
    }
    Ok(())
}

async fn choose_sleep_timer(
    window: &ApplicationWindow,
    title: &str,
    current: SleepTimer,
) -> Option<SleepTimer> {
    let dialog = adw::AlertDialog::new(
        Some(title),
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
    if dialog.choose_future(Some(window)).await != "apply" {
        return None;
    }
    entry.text().parse().ok().and_then(SleepTimer::from_minutes)
}
