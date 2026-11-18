use super::super::*;

pub(crate) fn register_brightness_action(window: &ApplicationWindow, ctx: &UiCtx) {
    let action = gtk::gio::SimpleAction::new("brightness", None);
    let parent = window.downgrade();
    let ctx = ctx.clone();
    action.connect_activate(move |action, _| {
        let Some(window) = parent.upgrade() else {
            return;
        };
        action.set_enabled(false);
        let action = action.clone();
        let ctx = ctx.clone();
        gtk::glib::spawn_future_local(async move {
            match edit_brightness(&window, &ctx).await {
                Ok(None) => {}
                Ok(Some(message)) | Err(message) => announce_status(&ctx.widgets, &message),
            }
            action.set_enabled(true);
        });
    });
    window.add_action(&action);
}

async fn edit_brightness(
    window: &ApplicationWindow,
    ctx: &UiCtx,
) -> Result<Option<String>, String> {
    let (path, profile) = {
        let state = ctx.state.borrow();
        (state.config_path.clone(), state.profile.clone())
    };
    let current = load_config(&path)?.brightness.min(100);
    let title = tr("Brightness");
    let dialog = adw::AlertDialog::new(Some(&title), None);
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("apply", &tr("Apply"));
    dialog.set_close_response("cancel");
    dialog.set_default_response(Some("apply"));
    dialog.set_response_appearance("apply", ResponseAppearance::Suggested);

    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_draw_value(true);
    scale.set_digits(0);
    scale.set_format_value_func(|_, value| format!("{value:.0}%"));
    scale.set_value(current as f64);
    scale.set_tooltip_text(Some(&title));
    dialog.set_extra_child(Some(&scale));

    if dialog.choose_future(Some(window)).await != "apply" || ctx.state.borrow().config_path != path
    {
        return Ok(None);
    }
    let value = scale.value().round() as u8;
    let message = gtk::gio::spawn_blocking(move || {
        streamrs::streamdeck::set_profile_brightness(&path, &profile, value)
    })
    .await
    .map_err(|_| tr("Could not change brightness."))??;
    Ok(Some(message))
}
