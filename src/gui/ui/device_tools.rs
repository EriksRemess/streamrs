use super::*;
use streamrs::streamdeck::{DisplayFill, firmware_image, info::open_info_device};

enum DeviceCommand {
    Fill(DisplayFill),
    Compare(PathBuf),
}

pub(super) fn register(window: &ApplicationWindow, state: &Rc<RefCell<AppState>>) {
    for name in ["display-color", "compare-firmware"] {
        let action = gtk::gio::SimpleAction::new(name, None);
        let parent = window.downgrade();
        let state = Rc::clone(state);
        action.connect_activate(move |action, _| {
            let Some(window) = parent.upgrade() else {
                return;
            };
            let (vendor, product, usage, page) = {
                let state = state.borrow();
                let config = &state.config;
                (
                    config.vendor_id,
                    config.product_id,
                    config.usage,
                    config.usage_page,
                )
            };
            let action = action.clone();
            action.set_enabled(false);
            gtk::glib::spawn_future_local(async move {
                let title = if name == "display-color" {
                    tr("Test display color")
                } else {
                    tr("Compare firmware image")
                };
                let result = async {
                    let command = if name == "display-color" {
                        choose_fill(&window, &title).await?.map(DeviceCommand::Fill)
                    } else {
                        let filter = gtk::FileFilter::new();
                        filter.set_name(Some(&tr("Firmware images")));
                        filter.add_suffix("bin");
                        choose_local_file(&window, &title, &filter)
                            .await?
                            .map(DeviceCommand::Compare)
                    };
                    let Some(command) = command else {
                        return Ok(None);
                    };
                    gtk::gio::spawn_blocking(move || {
                        let device = open_info_device(vendor, product, usage, page)?;
                        match command {
                            DeviceCommand::Fill(fill) => {
                                fill.apply(&device)?;
                                Ok(None)
                            }
                            DeviceCommand::Compare(path) => {
                                Ok(Some(firmware_image::compare(&device, &path)?.summary()))
                            }
                        }
                    })
                    .await
                    .map_err(|_| tr("Device command failed."))?
                }
                .await;
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

async fn choose_fill(
    window: &ApplicationWindow,
    title: &str,
) -> Result<Option<DisplayFill>, String> {
    let dialog = adw::AlertDialog::new(
        Some(title),
        Some(&tr("The next redraw can replace this temporary color.")),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("apply", &tr("Apply"));
    dialog.set_close_response("cancel");
    dialog.set_response_appearance("apply", ResponseAppearance::Suggested);

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
    dialog.set_extra_child(Some(&controls));

    if dialog.choose_future(Some(window)).await != "apply" {
        return Ok(None);
    }
    let rgba = color.rgba();
    let rgb = [rgba.red(), rgba.green(), rgba.blue()].map(|v| (v * 255.0).round() as u8);
    let fill = match target.selected() {
        0 => DisplayFill::lcd(rgb),
        number => DisplayFill::key(u8::try_from(number).map_err(|_| tr("Select a key."))?, rgb)?,
    };
    Ok(Some(fill))
}
