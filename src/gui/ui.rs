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

fn string_list_expression() -> gtk::PropertyExpression {
    gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<&gtk::Expression>,
        "string",
    )
}

fn combo_row_from_strings(title: &str, items: &[String]) -> ComboRow {
    let names: Vec<&str> = items.iter().map(String::as_str).collect();
    let model = gtk::StringList::new(&names);
    let row = ComboRow::new();
    row.set_title(title);
    row.set_model(Some(&model));
    row.set_expression(Some(string_list_expression()));
    row
}

fn icon_add_button() -> Button {
    let button = Button::builder()
        .icon_name("list-add-symbolic")
        .build();
    button.set_tooltip_text(Some(&tr("Add icon")));
    button.add_css_class("icon-add-button");
    button.set_size_request(UI_CONTROL_HEIGHT, UI_CONTROL_HEIGHT);
    button.set_halign(Align::Center);
    button.set_valign(Align::Center);
    button
}

fn stacked_control_row(title: &str, control: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(14);
    content.set_margin_end(14);

    let label = Label::new(Some(title));
    label.set_halign(Align::Start);
    label.set_wrap(true);
    label.add_css_class("field-label");

    control.as_ref().set_halign(Align::End);
    control.as_ref().set_valign(Align::Center);

    content.append(&label);
    content.append(control);
    row.set_child(Some(&content));
    row
}

fn stacked_selector_preview_row(
    title: &str,
    control: &impl IsA<gtk::Widget>,
    preview: &Picture,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);

    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(14);
    content.set_margin_end(14);

    let label = Label::new(Some(title));
    label.set_halign(Align::Start);
    label.set_wrap(true);
    label.add_css_class("field-label");

    control.as_ref().set_hexpand(true);
    control.as_ref().set_halign(Align::Fill);
    control.as_ref().set_valign(Align::Center);

    let controls = GtkBox::new(Orientation::Vertical, 10);
    controls.set_halign(Align::Fill);
    controls.set_valign(Align::Center);

    preview.set_halign(Align::End);
    preview.set_valign(Align::Center);

    controls.append(control);
    controls.append(preview);
    content.append(&label);
    content.append(&controls);
    row.set_child(Some(&content));
    row
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
    add_key_button.set_tooltip_text(Some(&tr("Add a button")));
    let add_icon_button = icon_add_button();
    let add_status_on_icon_button = icon_add_button();
    let add_status_off_icon_button = icon_add_button();
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
    inspector_panel.add_css_class("deck-card");
    inspector_panel.add_css_class("inspector-panel");

    let action_mode_labels = vec![tr("None"), tr("Launch command"), tr("Keyboard shortcut")];
    let action_type_dropdown = combo_row_from_strings(&tr("Action type"), &action_mode_labels);

    let action_entry = EntryRow::new();
    action_entry.set_title(&tr("Launch command"));

    let shortcut_entry = EntryRow::new();
    shortcut_entry.set_title(&tr("Keyboard shortcut"));

    let mode_labels = vec![
        tr("Blank"),
        tr("Regular"),
        tr("Status"),
        tr("Clock"),
        tr("Calendar"),
    ];
    let icon_kind_dropdown = combo_row_from_strings(&tr("Button type"), &mode_labels);

    let icon_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_dropdown);
    icon_dropdown.set_hexpand(true);
    icon_dropdown.set_size_request(-1, -1);
    let icon_controls = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    icon_controls.set_hexpand(true);
    icon_controls.set_halign(Align::Fill);
    icon_controls.append(&icon_dropdown);
    icon_controls.append(&add_icon_button);

    let icon_preview = Picture::new();
    icon_preview.set_size_request(104, 104);
    icon_preview.add_css_class("icon-preview");
    let icon_row = stacked_selector_preview_row(&tr("Icon"), &icon_controls, &icon_preview);

    let clock_background_dropdown = {
        let backgrounds = clock_backgrounds.borrow();
        dropdown_with_icons(&state, backgrounds.as_slice())
    };
    make_dropdown_shrinkable(&clock_background_dropdown);
    let clock_background_preview = Picture::new();
    clock_background_preview.set_size_request(104, 104);
    clock_background_preview.add_css_class("icon-preview");
    let clock_background_row = stacked_selector_preview_row(
        &tr("Clock background"),
        &clock_background_dropdown,
        &clock_background_preview,
    );

    let status_entry = EntryRow::new();
    status_entry.set_title(&tr("Status command"));

    let icon_on_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_on_dropdown);
    icon_on_dropdown.set_hexpand(true);
    icon_on_dropdown.set_size_request(-1, -1);
    let icon_on_preview = Picture::new();
    icon_on_preview.set_size_request(104, 104);
    icon_on_preview.add_css_class("icon-preview");
    let icon_on_controls = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    icon_on_controls.set_hexpand(true);
    icon_on_controls.set_halign(Align::Fill);
    icon_on_controls.append(&icon_on_dropdown);
    icon_on_controls.append(&add_status_on_icon_button);
    let icon_on_row = stacked_selector_preview_row(
        &tr("Icon when status is on"),
        &icon_on_controls,
        &icon_on_preview,
    );

    let icon_off_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_off_dropdown);
    icon_off_dropdown.set_hexpand(true);
    icon_off_dropdown.set_size_request(-1, -1);
    let icon_off_preview = Picture::new();
    icon_off_preview.set_size_request(104, 104);
    icon_off_preview.add_css_class("icon-preview");
    let icon_off_controls = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    icon_off_controls.set_hexpand(true);
    icon_off_controls.set_halign(Align::Fill);
    icon_off_controls.append(&icon_off_dropdown);
    icon_off_controls.append(&add_status_off_icon_button);
    let icon_off_row = stacked_selector_preview_row(
        &tr("Icon when status is off"),
        &icon_off_controls,
        &icon_off_preview,
    );

    let interval_spin = SpinButton::with_range(
        MIN_STATUS_INTERVAL_SECONDS as f64,
        MAX_STATUS_INTERVAL_SECONDS as f64,
        1.0,
    );
    interval_spin.set_valign(Align::Center);
    interval_spin.set_value(DEFAULT_STATUS_INTERVAL_SECONDS as f64);
    let interval_row = stacked_control_row(&tr("Status interval (seconds)"), &interval_spin);

    let apply_button = Button::with_label(&tr("Save and Apply"));
    apply_button.add_css_class("suggested-action");
    apply_button.set_hexpand(false);
    let clear_button = Button::with_label(&tr("Remove selected"));
    clear_button.set_tooltip_text(Some(&tr("Remove selected button configuration")));
    clear_button.add_css_class("destructive-action");
    clear_button.set_hexpand(false);

    let behavior_group = PreferencesGroup::builder().title(tr("Behavior")).build();
    behavior_group.set_margin_bottom(8);
    behavior_group.add(&action_type_dropdown);
    behavior_group.add(&action_entry);
    behavior_group.add(&shortcut_entry);

    let appearance_group = PreferencesGroup::builder().title(tr("Appearance")).build();
    appearance_group.set_margin_bottom(8);
    appearance_group.add(&icon_kind_dropdown);
    appearance_group.add(&icon_row);
    appearance_group.add(&icon_on_row);
    appearance_group.add(&icon_off_row);
    appearance_group.add(&clock_background_row);

    let status_group = PreferencesGroup::builder().title(tr("Status")).build();
    status_group.add(&status_entry);
    status_group.add(&interval_row);

    let editor_groups = GtkBox::new(Orientation::Vertical, 0);
    editor_groups.set_hexpand(true);
    editor_groups.set_vexpand(true);
    editor_groups.append(&behavior_group);
    editor_groups.append(&appearance_group);
    editor_groups.append(&status_group);

    let editor_content = GtkBox::new(Orientation::Vertical, UI_SPACING);
    editor_content.set_hexpand(true);
    editor_content.set_vexpand(true);
    editor_content.append(&editor_groups);

    editor_scroller.set_child(Some(&editor_content));
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
    let toast_overlay = ToastOverlay::new();

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
        let toast_overlay_for_start = toast_overlay.clone();
        let start_daemon_action_for_start = start_daemon_action.clone();
        let stop_daemon_action_for_start = stop_daemon_action.clone();
        start_daemon_action.connect_activate(move |_, _| {
            let message = match set_daemon_running(true) {
                Ok(()) => tr("Started streamrs daemon"),
                Err(err) => err,
            };
            let toast = Toast::new(&message);
            toast.set_timeout(3);
            toast_overlay_for_start.add_toast(toast);
            sync_daemon_actions(
                &start_daemon_action_for_start,
                &stop_daemon_action_for_start,
            );
        });
    }
    {
        let toast_overlay_for_stop = toast_overlay.clone();
        let start_daemon_action_for_stop = start_daemon_action.clone();
        let stop_daemon_action_for_stop = stop_daemon_action.clone();
        stop_daemon_action.connect_activate(move |_, _| {
            let message = match set_daemon_running(false) {
                Ok(()) => tr("Stopped streamrs daemon"),
                Err(err) => err,
            };
            let toast = Toast::new(&message);
            toast.set_timeout(3);
            toast_overlay_for_stop.add_toast(toast);
            sync_daemon_actions(&start_daemon_action_for_stop, &stop_daemon_action_for_stop);
        });
    }
    {
        let toast_overlay_for_restart = toast_overlay.clone();
        let start_daemon_action_for_restart = start_daemon_action.clone();
        let stop_daemon_action_for_restart = stop_daemon_action.clone();
        restart_daemon_action.connect_activate(move |_, _| {
            let message = match restart_daemon() {
                Ok(()) => tr("Restarted streamrs daemon"),
                Err(err) => err,
            };
            let toast = Toast::new(&message);
            toast.set_timeout(3);
            toast_overlay_for_restart.add_toast(toast);
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
    status_bar.set_hexpand(true);
    let button_actions = GtkBox::new(Orientation::Horizontal, UI_SPACING_HORIZONTAL);
    button_actions.add_css_class("status-button-actions");
    button_actions.set_halign(Align::Start);
    button_actions.append(&add_key_button);
    button_actions.append(&clear_button);
    let status_actions_spacer = GtkBox::new(Orientation::Horizontal, 0);
    status_actions_spacer.set_hexpand(true);
    status_bar.append(&button_actions);
    status_bar.append(&status_actions_spacer);
    status_bar.append(&apply_button);
    content_root.append(&body);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header_bar);
    toolbar_view.set_content(Some(&content_root));
    toolbar_view.add_bottom_bar(&status_bar);
    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    let widgets = EditorWidgets {
        profile_dropdown,
        profile_names,
        toast_overlay,
        action_type_dropdown,
        action_entry,
        shortcut_entry,
        icon_kind_dropdown,
        icon_row,
        icon_dropdown,
        clock_background_row,
        clock_background_dropdown,
        clock_background_preview,
        status_group,
        status_entry,
        icon_on_row,
        icon_on_dropdown,
        icon_on_preview,
        icon_off_row,
        icon_off_dropdown,
        icon_off_preview,
        interval_spin,
        icon_preview,
        apply_button: apply_button.clone(),
        clear_button: clear_button.clone(),
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
        vec![
            add_icon_button.clone(),
            add_status_on_icon_button.clone(),
            add_status_off_icon_button.clone(),
        ],
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
