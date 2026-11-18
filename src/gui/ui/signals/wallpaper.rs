use super::super::*;

pub(crate) fn register_wallpaper_action(window: &ApplicationWindow, ctx: &UiCtx) {
    let action = gtk::gio::SimpleAction::new("page-wallpaper", None);
    let parent = window.downgrade();
    let ctx = ctx.clone();
    action.connect_activate(move |action, _| {
        let Some(window) = parent.upgrade() else {
            return;
        };
        if ctx.state.borrow().profile == BLANK_PROFILE {
            announce_status(
                &ctx.widgets,
                &tr("Create a profile to set a page wallpaper."),
            );
            return;
        }
        action.set_enabled(false);
        let action = action.clone();
        let ctx = ctx.clone();
        gtk::glib::spawn_future_local(async move {
            if let Err(err) = edit_wallpaper(&window, &ctx).await {
                announce_status(&ctx.widgets, &err);
            }
            action.set_enabled(true);
        });
    });
    window.add_action(&action);
}

async fn edit_wallpaper(window: &ApplicationWindow, ctx: &UiCtx) -> Result<(), String> {
    let page = ctx.current_page.get();
    let (config_path, current) = {
        let state = ctx.state.borrow();
        (
            state.config_path.clone(),
            state.config.page_wallpaper(page).map(Path::to_path_buf),
        )
    };
    let title = trf(
        "Wallpaper for page {page}",
        &[("page", (page + 1).to_string())],
    );
    let dialog = adw::AlertDialog::new(
        Some(&title),
        current.as_deref().map(Path::to_string_lossy).as_deref(),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    if current.is_some() {
        dialog.add_response("remove", &tr("Remove wallpaper"));
    }
    dialog.add_response("choose", &tr("Choose image…"));
    dialog.set_close_response("cancel");
    let chosen = match dialog.choose_future(Some(window)).await.as_str() {
        "remove" => None,
        "choose" => {
            let chooser = gtk::FileChooserNative::builder()
                .title(&title)
                .transient_for(window)
                .modal(true)
                .action(gtk::FileChooserAction::Open)
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&tr("Images")));
            for mime in ["image/png", "image/jpeg", "image/webp"] {
                filter.add_mime_type(mime);
            }
            chooser.add_filter(&filter);
            if chooser.run_future().await != gtk::ResponseType::Accept {
                return Ok(());
            }
            let path = chooser
                .file()
                .and_then(|f| f.path())
                .ok_or_else(|| tr("Select a local image file."))?;
            Some(
                gtk::gio::spawn_blocking(move || CachedWallpaper::load(path))
                    .await
                    .map_err(|_| tr("Could not load the wallpaper."))??,
            )
        }
        _ => return Ok(()),
    };

    if ctx.state.borrow().config_path != config_path {
        return Ok(());
    }

    // Read the saved profile so changing a wallpaper does not commit pending key edits.
    let mut config = load_config(&config_path)?;
    let page_key = (page + 1).to_string();
    if let Some(wallpaper) = &chosen {
        config
            .page_wallpapers
            .insert(page_key, wallpaper.path.clone());
    } else {
        config.page_wallpapers.remove(&page_key);
    }
    save_config(&config_path, &config)?;
    {
        let mut state = ctx.state.borrow_mut();
        state.config.page_wallpapers = config.page_wallpapers;
        state.wallpaper_cache = chosen.map(Rc::new);
    }
    refresh_key_grid(
        &ctx.state,
        &ctx.key_buttons,
        &ctx.key_pictures,
        ctx.current_page.get(),
        &ctx.clock_backgrounds.borrow(),
    );
    if daemon_running() {
        signal_daemon_reload()?;
    }
    Ok(())
}
