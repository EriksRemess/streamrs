use crate::gui::*;

mod signal_wiring;
use signal_wiring::wire_ui_handlers_and_present;

fn fallback_non_blank_profile(profiles: &[String]) -> Option<String> {
    profiles
        .iter()
        .find(|profile| profile.as_str() == DEFAULT_PROFILE)
        .cloned()
        .or_else(|| profiles.first().cloned())
}

fn choose_startup_profile(profiles: &[String], current_profile: Option<String>) -> String {
    if let Some(current_profile) = current_profile {
        if current_profile != BLANK_PROFILE {
            return current_profile;
        }
        if profiles.is_empty() {
            return BLANK_PROFILE.to_string();
        }
    }

    if profiles.is_empty() {
        return BLANK_PROFILE.to_string();
    }

    fallback_non_blank_profile(profiles).unwrap_or_else(|| BLANK_PROFILE.to_string())
}

fn startup_profile_names(
    mut discovered_profiles: Vec<String>,
    startup_profile: &str,
) -> Vec<String> {
    if startup_profile != BLANK_PROFILE
        && !discovered_profiles
            .iter()
            .any(|profile| profile == startup_profile)
    {
        discovered_profiles.push(startup_profile.to_string());
    }
    discovered_profiles.sort_unstable();
    discovered_profiles.dedup();
    discovered_profiles
}

fn present_about_dialog(parent: &ApplicationWindow) {
    let dialog = adw::AboutDialog::new();
    dialog.set_application_icon("lv.apps.streamrs");
    dialog.set_application_name("streamrs");
    dialog.set_version(env!("CARGO_PKG_VERSION"));
    dialog.set_comments(&tr("A lightweight Rust Stream Deck toolkit for Linux."));
    dialog.set_developer_name("Ēriks Remess");
    dialog.set_developers(&["Ēriks Remess <eriks@remess.lv>"]);
    dialog.set_website("https://github.com/EriksRemess/streamrs");
    dialog.set_issue_url("https://github.com/EriksRemess/streamrs/issues");
    dialog.set_copyright("Copyright (c) 2026 Ēriks Remess");
    dialog.set_license_type(gtk::License::MitX11);
    dialog.add_link(
        &tr("Contributors"),
        "https://github.com/EriksRemess/streamrs/graphs/contributors",
    );
    dialog.add_link(
        &tr("Project License"),
        "https://github.com/EriksRemess/streamrs/blob/main/LICENSE",
    );
    dialog.present(Some(parent));
}

fn rebuild_window_menu(menu: &gtk::gio::Menu) {
    menu.remove_all();

    let daemon_section = gtk::gio::Menu::new();
    daemon_section.append(Some(&tr("Start")), Some("win.start-daemon"));
    daemon_section.append(Some(&tr("Stop")), Some("win.stop-daemon"));
    daemon_section.append(Some(&tr("Restart")), Some("win.restart-daemon"));
    menu.append_section(Some(&tr("streamrs service")), &daemon_section);

    let app_section = gtk::gio::Menu::new();
    app_section.append(Some(&tr("About streamrs")), Some("win.show-about"));
    menu.append_section(None, &app_section);
}

fn sync_daemon_actions(
    start_action: &gtk::gio::SimpleAction,
    stop_action: &gtk::gio::SimpleAction,
) {
    let running = daemon_running();
    start_action.set_enabled(!running);
    stop_action.set_enabled(running);
}

pub(crate) fn build_ui(app: &Application) {
    if let Some(existing_window) = app.windows().first() {
        existing_window.present();
        return;
    }

    install_css();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let discovered_profiles = discover_profiles();
    let (startup_current, should_save_startup_profile) = match load_current_profile() {
        Ok(profile) => {
            let should_save = profile.is_none();
            (profile, should_save)
        }
        Err(err) => {
            eprintln!("{err}");
            (None, false)
        }
    };
    let default_profile = choose_startup_profile(&discovered_profiles, startup_current);
    let profiles = startup_profile_names(discovered_profiles, &default_profile);
    let default_config_path = default_config_path_for_profile(&default_profile);
    let (writable_image_dir, image_dirs) = image_paths_for_profile(&default_profile);
    let deck_image_path = manifest_dir.join("scripts").join("streamdeck.svg");
    let app_icon_path = manifest_dir.join("config").join("lv.apps.streamrs.png");

    let catalog_dirs = image_dirs.clone();
    let icons = discover_icons(&catalog_dirs);
    let icon_names = Rc::new(RefCell::new(icons));
    let clock_backgrounds = Rc::new(RefCell::new(discover_clock_backgrounds(&catalog_dirs)));

    let initial_config = match load_config(&default_config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            if default_profile == BLANK_PROFILE {
                streamrs::config::streamrs_schema::blank_profile_config()
            } else {
                Config::default()
            }
        }
    };
    if should_save_startup_profile && let Err(err) = save_current_profile(&default_profile) {
        eprintln!("{err}");
    }
    if default_profile == BLANK_PROFILE
        && !default_config_path.is_file()
        && let Err(err) = save_config(&default_config_path, &initial_config)
    {
        eprintln!("{err}");
    }

    let state = Rc::new(RefCell::new(AppState {
        config: initial_config,
        config_path: default_config_path.clone(),
        profile: default_profile,
        image_dirs,
        writable_image_dir,
    }));
    let profile_names = Rc::new(RefCell::new(profiles));
    let selected_key = Rc::new(Cell::new(0usize));
    let current_page = Rc::new(Cell::new(0usize));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("streamrs")
        .icon_name("lv.apps.streamrs")
        .default_width(WINDOW_MIN_WIDTH)
        .default_height(WINDOW_MIN_HEIGHT)
        .build();
    window.set_size_request(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT);

    let content_root = GtkBox::new(Orientation::Vertical, 0);
    content_root.add_css_class("streamrs-root");

    let profile_dropdown = {
        let profiles = profile_names.borrow();
        let labels: Vec<String> = profiles
            .iter()
            .map(|profile| profile_display_name(profile))
            .collect();
        let names: Vec<&str> = labels.iter().map(String::as_str).collect();
        DropDown::from_strings(&names)
    };
    profile_dropdown.set_hexpand(false);
    profile_dropdown.set_size_request(PROFILE_DROPDOWN_WIDTH, -1);
    profile_dropdown.add_css_class("streamrs-field");
    if let Some(initial_profile_index) = profile_names
        .borrow()
        .iter()
        .position(|profile| profile == &state.borrow().profile)
    {
        profile_dropdown.set_selected(initial_profile_index as u32);
    }
    let add_profile_button = Button::with_label(&tr("Add"));
    let remove_profile_button = Button::with_label(&tr("Remove"));
    let rename_profile_button = Button::with_label(&tr("Rename"));
    add_profile_button.add_css_class("profile-action-button");
    remove_profile_button.add_css_class("profile-action-button");
    rename_profile_button.add_css_class("profile-action-button");

    let add_key_button = Button::with_label(&tr("Add a button"));
    let add_icon_button = Button::with_label("+");
    add_icon_button.set_tooltip_text(Some(&tr("Add icon")));
    add_icon_button.add_css_class("icon-add-button");
    add_icon_button.set_size_request(UI_CONTROL_HEIGHT, UI_CONTROL_HEIGHT);
    add_icon_button.set_halign(Align::Center);
    add_icon_button.set_valign(Align::Center);
    let has_profiles = !profile_names.borrow().is_empty();
    profile_dropdown.set_sensitive(has_profiles);
    remove_profile_button.set_sensitive(has_profiles);
    rename_profile_button.set_sensitive(has_profiles);

    let body = Paned::new(Orientation::Horizontal);
    body.add_css_class("main-split");
    body.set_wide_handle(true);
    body.set_shrink_start_child(false);
    body.set_shrink_end_child(false);
    body.set_resize_start_child(true);
    body.set_resize_end_child(false);
    let compact_left_width =
        (WINDOW_MIN_WIDTH - INSPECTOR_MIN_WIDTH - (UI_SPACING * 3)).max(DECK_MIN_WIDTH);
    body.set_position(compact_left_width);

    let left_panel = GtkBox::new(Orientation::Vertical, UI_SPACING);
    left_panel.set_hexpand(true);
    left_panel.set_vexpand(true);
    left_panel.set_size_request(DECK_MIN_WIDTH + (UI_SPACING * 2) + 8, -1);
    left_panel.add_css_class("deck-card");

    let deck_label = Label::new(Some(&tr("Stream Deck preview")));
    deck_label.set_halign(Align::Start);
    deck_label.add_css_class("section-title");
    let deck_header = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    let deck_header_spacer = GtkBox::new(Orientation::Horizontal, 0);
    deck_header_spacer.set_hexpand(true);
    let prev_page_button = Button::with_label(&tr("Prev"));
    prev_page_button.add_css_class("flat");
    prev_page_button.set_visible(false);
    let next_page_button = Button::with_label(&tr("Next"));
    next_page_button.add_css_class("flat");
    next_page_button.set_visible(false);
    add_key_button.add_css_class("action-button");
    let page_label = Label::new(Some(&trf(
        "Page {current}/{total}",
        &[("current", "1".to_string()), ("total", "1".to_string())],
    )));
    page_label.add_css_class("page-indicator");
    page_label.set_valign(Align::Start);
    deck_label.set_valign(Align::Start);
    deck_header.append(&deck_label);
    deck_header.append(&deck_header_spacer);
    deck_header.append(&page_label);

    let deck_overlay = Overlay::new();
    deck_overlay.set_halign(Align::Fill);
    deck_overlay.set_valign(Align::Fill);
    deck_overlay.set_hexpand(true);
    deck_overlay.set_vexpand(true);
    deck_overlay.set_size_request(DECK_MIN_WIDTH, DECK_MIN_HEIGHT);

    let deck_picture = Picture::new();
    deck_picture.set_keep_aspect_ratio(true);
    deck_picture.set_can_shrink(true);
    deck_picture.add_css_class("deck-image");
    if let Some(background_path) =
        write_deck_background_png(&deck_image_path, PREVIEW_WIDTH, PREVIEW_HEIGHT)
    {
        update_picture_file(&deck_picture, Some(&background_path));
    } else if deck_image_path.is_file() {
        update_picture_file(&deck_picture, Some(&deck_image_path));
    }

    deck_overlay.set_child(Some(&deck_picture));

    let key_layer = Fixed::new();
    deck_overlay.add_overlay(&key_layer);

    let slots = key_slots_for_deck(&deck_image_path);

    let mut key_buttons = Vec::with_capacity(KEY_COUNT);
    let mut key_pictures = Vec::with_capacity(KEY_COUNT);

    for (index, slot) in slots.iter().copied().enumerate().take(KEY_COUNT) {
        let button = Button::new();
        button.add_css_class("key-button");
        button.set_tooltip_text(Some(&trf(
            "Button {index}",
            &[("index", (index + 1).to_string())],
        )));

        let width = (slot.x1 - slot.x0) as i32;
        let height = (slot.y1 - slot.y0) as i32;
        button.set_size_request(width, height);

        let picture = Picture::new();
        picture.set_size_request(width.saturating_sub(10), height.saturating_sub(10));
        picture.set_keep_aspect_ratio(true);
        picture.set_can_shrink(true);
        picture.set_halign(Align::Center);
        picture.set_valign(Align::Center);
        picture.add_css_class("key-icon");
        button.set_child(Some(&picture));

        key_layer.put(&button, slot.x0 as f64, slot.y0 as f64);

        key_buttons.push(button);
        key_pictures.push(picture);
    }

    left_panel.append(&deck_header);
    left_panel.append(&deck_overlay);

    {
        let deck_picture_for_layout = deck_picture.clone();
        let key_layer_for_layout = key_layer.clone();
        let slots_for_layout = slots.clone();
        let key_buttons_for_layout = key_buttons.clone();
        let key_pictures_for_layout = key_pictures.clone();
        let last_size = Rc::new(Cell::new((0i32, 0i32)));
        let last_size_for_tick = last_size.clone();

        deck_overlay.add_tick_callback(move |overlay, _clock| {
            let size = (overlay.allocated_width(), overlay.allocated_height());
            if size.0 <= 0 || size.1 <= 0 {
                return gtk::glib::ControlFlow::Continue;
            }

            if size != last_size_for_tick.get() {
                last_size_for_tick.set(size);
                relayout_deck(
                    overlay,
                    &deck_picture_for_layout,
                    &key_layer_for_layout,
                    &slots_for_layout,
                    &key_buttons_for_layout,
                    &key_pictures_for_layout,
                );
            }

            gtk::glib::ControlFlow::Continue
        });
    }

    let editor_scroller = ScrolledWindow::new();
    editor_scroller.add_css_class("inspector-scroller");
    editor_scroller.set_vexpand(true);
    editor_scroller.set_hexpand(true);
    editor_scroller.set_min_content_width(INSPECTOR_MIN_WIDTH);
    editor_scroller.set_overlay_scrolling(true);
    editor_scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    editor_scroller.set_margin_end(0);

    let inspector_panel = GtkBox::new(Orientation::Vertical, 0);
    inspector_panel.set_hexpand(true);
    inspector_panel.set_vexpand(true);
    inspector_panel.add_css_class("inspector-card");

    let editor = GtkBox::new(Orientation::Vertical, UI_SPACING);
    editor.set_hexpand(true);
    editor.set_margin_top(0);
    editor.set_margin_bottom(0);
    editor.set_margin_start(0);
    editor.set_margin_end(0);

    let selected_label = Label::new(Some(&trf(
        "Editing {ordinal} button",
        &[("ordinal", tr_ordinal(1))],
    )));
    selected_label.set_halign(Align::Start);
    selected_label.add_css_class("section-title");

    let action_type_label = Label::new(Some(&tr("Action type")));
    action_type_label.set_halign(Align::Start);
    action_type_label.add_css_class("field-label");
    let action_mode_labels = vec![tr("None"), tr("Launch command"), tr("Keyboard shortcut")];
    let action_mode_label_refs: Vec<&str> = action_mode_labels.iter().map(String::as_str).collect();
    let action_type_dropdown = DropDown::from_strings(&action_mode_label_refs);
    make_dropdown_shrinkable(&action_type_dropdown);
    action_type_dropdown.add_css_class("streamrs-field");

    let action_label = Label::new(Some(&tr("Launch command")));
    action_label.set_halign(Align::Start);
    action_label.add_css_class("field-label");
    let action_entry = Entry::new();
    action_entry.set_hexpand(true);
    action_entry.set_width_chars(1);
    action_entry.set_placeholder_text(Some("open https://example.com"));
    action_entry.add_css_class("streamrs-field");

    let shortcut_label = Label::new(Some(&tr("Keyboard shortcut")));
    shortcut_label.set_halign(Align::Start);
    shortcut_label.add_css_class("field-label");
    let shortcut_entry = Entry::new();
    shortcut_entry.set_hexpand(true);
    shortcut_entry.set_width_chars(1);
    shortcut_entry.set_placeholder_text(Some("Ctrl+Shift+T"));
    shortcut_entry.add_css_class("streamrs-field");

    let icon_kind_label = Label::new(Some(&tr("Button type")));
    icon_kind_label.set_halign(Align::Start);
    icon_kind_label.add_css_class("field-label");
    let mode_labels = vec![
        tr("Blank"),
        tr("Regular"),
        tr("Status"),
        tr("Clock"),
        tr("Calendar"),
    ];
    let mode_label_refs: Vec<&str> = mode_labels.iter().map(String::as_str).collect();
    let icon_kind_dropdown = DropDown::from_strings(&mode_label_refs);
    make_dropdown_shrinkable(&icon_kind_dropdown);
    icon_kind_dropdown.add_css_class("streamrs-field");

    let icon_label = Label::new(Some(&tr("Icon")));
    icon_label.set_halign(Align::Start);
    icon_label.add_css_class("field-label");
    let icon_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_dropdown);
    icon_dropdown.add_css_class("streamrs-field");
    let icon_row = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    icon_row.set_hexpand(true);
    icon_row.add_css_class("icon-row");
    icon_row.append(&icon_dropdown);
    icon_row.append(&add_icon_button);

    let clock_background_label = Label::new(Some(&tr("Clock background")));
    clock_background_label.set_halign(Align::Start);
    clock_background_label.add_css_class("field-label");
    let clock_background_dropdown = {
        let backgrounds = clock_backgrounds.borrow();
        dropdown_with_icons(&state, backgrounds.as_slice())
    };
    make_dropdown_shrinkable(&clock_background_dropdown);
    clock_background_dropdown.add_css_class("streamrs-field");

    let icon_preview_label = Label::new(Some(&tr("Icon Preview")));
    icon_preview_label.set_halign(Align::Start);
    icon_preview_label.add_css_class("field-label");
    let icon_preview = Picture::new();
    icon_preview.set_size_request(104, 104);
    icon_preview.add_css_class("icon-preview");

    let status_command_label = Label::new(Some(&tr("Status command (optional)")));
    status_command_label.set_halign(Align::Start);
    status_command_label.add_css_class("field-label");
    let status_entry = Entry::new();
    status_entry.set_hexpand(true);
    status_entry.set_width_chars(1);
    status_entry.add_css_class("streamrs-field");

    let icon_on_label = Label::new(Some(&tr("Icon when status is on")));
    icon_on_label.set_halign(Align::Start);
    icon_on_label.add_css_class("field-label");
    let icon_on_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_on_dropdown);
    icon_on_dropdown.add_css_class("streamrs-field");

    let icon_off_label = Label::new(Some(&tr("Icon when status is off")));
    icon_off_label.set_halign(Align::Start);
    icon_off_label.add_css_class("field-label");
    let icon_off_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_off_dropdown);
    icon_off_dropdown.add_css_class("streamrs-field");

    let interval_label = Label::new(Some(&tr("Status interval (ms)")));
    interval_label.set_halign(Align::Start);
    interval_label.add_css_class("field-label");
    let interval_spin = SpinButton::with_range(
        MIN_STATUS_INTERVAL_MS as f64,
        MAX_STATUS_INTERVAL_MS as f64,
        100.0,
    );
    interval_spin.set_hexpand(true);
    interval_spin.set_value(DEFAULT_STATUS_INTERVAL_MS as f64);
    interval_spin.add_css_class("streamrs-field");

    let apply_button = Button::with_label(&tr("Save"));
    apply_button.add_css_class("action-button");
    apply_button.add_css_class("apply-button");
    apply_button.set_hexpand(false);
    let clear_button = Button::with_label(&tr("Delete"));
    clear_button.set_tooltip_text(Some(&tr("Delete selected button configuration")));
    clear_button.add_css_class("action-button");
    clear_button.add_css_class("clear-button");
    clear_button.set_hexpand(false);

    let status_line = Label::new(Some(&tr("Ready")));
    status_line.set_halign(Align::Start);
    status_line.add_css_class("status-label");

    editor.append(&selected_label);
    editor.append(&action_type_label);
    editor.append(&action_type_dropdown);
    editor.append(&action_label);
    editor.append(&action_entry);
    editor.append(&shortcut_label);
    editor.append(&shortcut_entry);
    editor.append(&icon_kind_label);
    editor.append(&icon_kind_dropdown);
    editor.append(&icon_label);
    editor.append(&icon_row);
    editor.append(&clock_background_label);
    editor.append(&clock_background_dropdown);
    editor.append(&icon_preview_label);
    editor.append(&icon_preview);
    editor.append(&status_command_label);
    editor.append(&status_entry);
    editor.append(&icon_on_label);
    editor.append(&icon_on_dropdown);
    editor.append(&icon_off_label);
    editor.append(&icon_off_dropdown);
    editor.append(&interval_label);
    editor.append(&interval_spin);

    editor_scroller.set_child(Some(&editor));
    inspector_panel.append(&editor_scroller);

    body.set_start_child(Some(&left_panel));
    body.set_end_child(Some(&inspector_panel));

    let header_bar = HeaderBar::new();
    header_bar.add_css_class("flat");
    header_bar.add_css_class("window-titlebar");
    header_bar.set_show_end_title_buttons(true);

    let window_menu = gtk::gio::Menu::new();
    rebuild_window_menu(&window_menu);

    let menu_button = gtk::MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");
    menu_button.set_tooltip_text(Some(&tr("Menu")));
    menu_button.set_menu_model(Some(&window_menu));
    menu_button.add_css_class("flat");

    let about_action = gtk::gio::SimpleAction::new("show-about", None);
    {
        let window_for_about = window.clone();
        about_action.connect_activate(move |_, _| {
            present_about_dialog(&window_for_about);
        });
    }
    window.add_action(&about_action);

    let start_daemon_action = gtk::gio::SimpleAction::new("start-daemon", None);
    let stop_daemon_action = gtk::gio::SimpleAction::new("stop-daemon", None);
    let restart_daemon_action = gtk::gio::SimpleAction::new("restart-daemon", None);
    sync_daemon_actions(&start_daemon_action, &stop_daemon_action);
    {
        let start_daemon_action_for_open = start_daemon_action.clone();
        let stop_daemon_action_for_open = stop_daemon_action.clone();
        menu_button.connect_notify_local(Some("active"), move |button, _| {
            if button.property::<bool>("active") {
                sync_daemon_actions(&start_daemon_action_for_open, &stop_daemon_action_for_open);
            }
        });
    }
    {
        let status_line_for_start = status_line.clone();
        let start_daemon_action_for_start = start_daemon_action.clone();
        let stop_daemon_action_for_start = stop_daemon_action.clone();
        start_daemon_action.connect_activate(move |_, _| {
            let message = match set_daemon_running(true) {
                Ok(()) => tr("Started streamrs daemon"),
                Err(err) => err,
            };
            status_line_for_start.set_text(&message);
            sync_daemon_actions(
                &start_daemon_action_for_start,
                &stop_daemon_action_for_start,
            );
        });
    }
    {
        let status_line_for_stop = status_line.clone();
        let start_daemon_action_for_stop = start_daemon_action.clone();
        let stop_daemon_action_for_stop = stop_daemon_action.clone();
        stop_daemon_action.connect_activate(move |_, _| {
            let message = match set_daemon_running(false) {
                Ok(()) => tr("Stopped streamrs daemon"),
                Err(err) => err,
            };
            status_line_for_stop.set_text(&message);
            sync_daemon_actions(&start_daemon_action_for_stop, &stop_daemon_action_for_stop);
        });
    }
    {
        let status_line_for_restart = status_line.clone();
        let start_daemon_action_for_restart = start_daemon_action.clone();
        let stop_daemon_action_for_restart = stop_daemon_action.clone();
        restart_daemon_action.connect_activate(move |_, _| {
            let message = match restart_daemon() {
                Ok(()) => tr("Restarted streamrs daemon"),
                Err(err) => err,
            };
            status_line_for_restart.set_text(&message);
            sync_daemon_actions(
                &start_daemon_action_for_restart,
                &stop_daemon_action_for_restart,
            );
        });
    }
    window.add_action(&start_daemon_action);
    window.add_action(&stop_daemon_action);
    window.add_action(&restart_daemon_action);

    let title_row = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    title_row.set_halign(Align::Start);
    let title_icon = if app_icon_path.is_file() {
        Image::from_file(app_icon_path)
    } else {
        Image::from_icon_name("lv.apps.streamrs")
    };
    title_icon.set_pixel_size(32);
    let title_label = Label::new(Some("streamrs"));
    title_label.add_css_class("header-title-label");
    title_row.append(&title_icon);
    title_row.append(&title_label);

    let profile_controls = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    profile_controls.set_halign(Align::Center);
    let profile_label = Label::new(Some(&tr("Profile")));
    profile_label.add_css_class("field-label");
    profile_controls.append(&profile_label);
    profile_controls.append(&profile_dropdown);
    profile_controls.append(&add_profile_button);
    profile_controls.append(&remove_profile_button);
    profile_controls.append(&rename_profile_button);

    header_bar.pack_start(&title_row);
    header_bar.set_title_widget(Some(&profile_controls));
    header_bar.pack_end(&menu_button);
    let status_bar = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    status_bar.add_css_class("status-bar");
    status_line.set_hexpand(true);
    status_line.set_halign(Align::Fill);
    status_line.set_xalign(0.0);
    status_bar.append(&status_line);
    let status_actions = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    status_actions.set_homogeneous(true);
    status_actions.append(&add_key_button);
    status_actions.append(&apply_button);
    status_actions.append(&clear_button);
    status_bar.append(&status_actions);
    content_root.append(&body);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&content_root));
    toolbar_view.add_bottom_bar(&status_bar);
    window.set_content(Some(&toolbar_view));

    let widgets = EditorWidgets {
        profile_dropdown,
        profile_names,
        selected_label,
        action_type_dropdown,
        action_label,
        action_entry,
        shortcut_label,
        shortcut_entry,
        icon_kind_dropdown,
        icon_label,
        icon_row,
        icon_dropdown,
        clock_background_label,
        clock_background_dropdown,
        status_command_label,
        status_entry,
        icon_on_label,
        icon_on_dropdown,
        icon_off_label,
        icon_off_dropdown,
        interval_label,
        interval_spin,
        icon_preview,
        apply_button: apply_button.clone(),
        clear_button: clear_button.clone(),
        status_label: status_line,
    };
    wire_ui_handlers_and_present(
        &window,
        state.clone(),
        current_page.clone(),
        selected_key.clone(),
        widgets.clone(),
        icon_names.clone(),
        clock_backgrounds.clone(),
        key_buttons.clone(),
        key_pictures.clone(),
        prev_page_button.clone(),
        next_page_button.clone(),
        page_label.clone(),
        add_profile_button.clone(),
        remove_profile_button.clone(),
        rename_profile_button.clone(),
        add_key_button.clone(),
        add_icon_button.clone(),
        apply_button.clone(),
        clear_button.clone(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_startup_profile_prefers_current_when_present() {
        let profiles = vec!["default".to_string(), "test".to_string()];
        let selected = choose_startup_profile(&profiles, Some("test".to_string()));
        assert_eq!(selected, "test");
    }

    #[test]
    fn choose_startup_profile_ignores_blank_when_real_profiles_exist() {
        let profiles = vec!["default".to_string(), "test".to_string()];
        let selected = choose_startup_profile(&profiles, Some(BLANK_PROFILE.to_string()));
        assert_eq!(selected, "default");
    }

    #[test]
    fn choose_startup_profile_uses_blank_only_when_no_profiles_exist() {
        let profiles = Vec::new();
        let selected = choose_startup_profile(&profiles, Some(BLANK_PROFILE.to_string()));
        assert_eq!(selected, BLANK_PROFILE);
    }

    #[test]
    fn choose_startup_profile_keeps_non_blank_when_not_discovered() {
        let profiles = vec!["default".to_string()];
        let selected = choose_startup_profile(&profiles, Some("work".to_string()));
        assert_eq!(selected, "work");
    }

    #[test]
    fn startup_profile_names_includes_selected_non_blank_profile() {
        let discovered = vec!["default".to_string()];
        let names = startup_profile_names(discovered, "work");
        assert_eq!(names, vec!["default".to_string(), "work".to_string()]);
    }
}
