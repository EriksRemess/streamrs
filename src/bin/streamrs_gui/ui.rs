include!("ui/signal_wiring.rs");

fn build_ui(app: &Application) {
    install_css();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let default_profile = "default".to_string();
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
            Config::default()
        }
    };

    let state = Rc::new(RefCell::new(AppState {
        config: initial_config,
        config_path: default_config_path.clone(),
        profile: default_profile,
        image_dirs,
        writable_image_dir,
    }));
    let selected_key = Rc::new(Cell::new(0usize));
    let current_page = Rc::new(Cell::new(0usize));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("streamrs")
        .icon_name("lv.apps.streamrs")
        .default_width(1480)
        .default_height(920)
        .build();
    window.set_size_request(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT);

    let root = GtkBox::new(Orientation::Vertical, 10);
    root.add_css_class("streamrs-root");

    let top_bar = GtkBox::new(Orientation::Horizontal, 10);
    top_bar.add_css_class("config-bar");
    let config_icon = Image::from_icon_name("document-open-symbolic");
    config_icon.set_pixel_size(16);
    let config_path_label = Label::new(Some("Config"));
    config_path_label.set_halign(Align::Start);
    config_path_label.add_css_class("field-label");

    let config_path_entry = Entry::new();
    config_path_entry.set_hexpand(true);
    config_path_entry.set_text(default_config_path.to_string_lossy().as_ref());
    config_path_entry.set_placeholder_text(Some("Path to streamrs profile config"));

    let load_button = Button::with_label("Load");
    let save_button = Button::with_label("Save");
    let add_key_button = Button::with_label("Add button");
    let add_icon_button = Button::with_label("+");
    add_icon_button.set_tooltip_text(Some("Add icon"));
    add_icon_button.add_css_class("icon-add-button");
    top_bar.append(&config_icon);
    top_bar.append(&config_path_label);
    top_bar.append(&config_path_entry);
    top_bar.append(&load_button);
    top_bar.append(&save_button);

    let body = Paned::new(Orientation::Horizontal);
    body.set_wide_handle(true);
    body.set_shrink_start_child(true);
    body.set_shrink_end_child(false);
    body.set_resize_start_child(true);
    body.set_resize_end_child(false);
    body.set_position((PREVIEW_WIDTH as i32) + 90);

    let left_panel = GtkBox::new(Orientation::Vertical, 10);
    left_panel.set_hexpand(true);
    left_panel.set_vexpand(true);
    left_panel.add_css_class("deck-card");

    let deck_label = Label::new(Some("Stream Deck layout"));
    deck_label.set_halign(Align::Start);
    deck_label.add_css_class("section-title");
    let deck_header = GtkBox::new(Orientation::Horizontal, 8);
    let deck_header_spacer = GtkBox::new(Orientation::Horizontal, 0);
    deck_header_spacer.set_hexpand(true);
    let prev_page_button = Button::with_label("Prev");
    prev_page_button.add_css_class("flat");
    prev_page_button.set_visible(false);
    let next_page_button = Button::with_label("Next");
    next_page_button.add_css_class("flat");
    next_page_button.set_visible(false);
    add_key_button.add_css_class("flat");
    let page_label = Label::new(Some("Page 1/1"));
    page_label.add_css_class("field-label");
    deck_header.append(&deck_label);
    deck_header.append(&deck_header_spacer);
    deck_header.append(&add_key_button);
    deck_header.append(&page_label);

    let deck_overlay = Overlay::new();
    deck_overlay.set_halign(Align::Fill);
    deck_overlay.set_valign(Align::Fill);
    deck_overlay.set_hexpand(true);
    deck_overlay.set_vexpand(true);

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

    for index in 0..KEY_COUNT {
        let slot = slots[index];

        let button = Button::new();
        button.add_css_class("key-button");
        button.set_tooltip_text(Some(&format!("Key {}", index + 1)));

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
    editor_scroller.set_vexpand(true);
    editor_scroller.set_hexpand(true);
    editor_scroller.set_min_content_width(INSPECTOR_MIN_WIDTH);
    editor_scroller.set_overlay_scrolling(true);

    let inspector_panel = GtkBox::new(Orientation::Vertical, 0);
    inspector_panel.set_hexpand(true);
    inspector_panel.set_vexpand(true);
    inspector_panel.add_css_class("inspector-card");

    let editor = GtkBox::new(Orientation::Vertical, 10);
    editor.set_hexpand(true);
    editor.set_margin_top(8);
    editor.set_margin_bottom(8);
    editor.set_margin_start(8);
    editor.set_margin_end(18);

    let selected_label = Label::new(Some("Editing key 1"));
    selected_label.set_halign(Align::Start);
    selected_label.add_css_class("section-title");

    let action_label = Label::new(Some("Action"));
    action_label.set_halign(Align::Start);
    action_label.add_css_class("field-label");
    let action_entry = Entry::new();
    action_entry.set_hexpand(true);
    action_entry.set_width_chars(1);

    let icon_kind_label = Label::new(Some("Icon type"));
    icon_kind_label.set_halign(Align::Start);
    icon_kind_label.add_css_class("field-label");
    let icon_kind_dropdown = DropDown::from_strings(&["Regular", "Status", "Clock"]);
    make_dropdown_shrinkable(&icon_kind_dropdown);

    let icon_label = Label::new(Some("Icon"));
    icon_label.set_halign(Align::Start);
    icon_label.add_css_class("field-label");
    let icon_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_dropdown);
    let icon_row = GtkBox::new(Orientation::Horizontal, 6);
    icon_row.set_hexpand(true);
    icon_row.append(&icon_dropdown);
    icon_row.append(&add_icon_button);

    let clock_background_label = Label::new(Some("Clock background"));
    clock_background_label.set_halign(Align::Start);
    clock_background_label.add_css_class("field-label");
    let clock_background_dropdown = {
        let backgrounds = clock_backgrounds.borrow();
        dropdown_with_icons(&state, backgrounds.as_slice())
    };
    make_dropdown_shrinkable(&clock_background_dropdown);

    let icon_preview_label = Label::new(Some("Icon Preview"));
    icon_preview_label.set_halign(Align::Start);
    icon_preview_label.add_css_class("field-label");
    let icon_preview = Picture::new();
    icon_preview.set_size_request(120, 120);
    icon_preview.add_css_class("icon-preview");

    let status_command_label = Label::new(Some("Status command (optional)"));
    status_command_label.set_halign(Align::Start);
    status_command_label.add_css_class("field-label");
    let status_entry = Entry::new();
    status_entry.set_hexpand(true);
    status_entry.set_width_chars(1);

    let icon_on_label = Label::new(Some("Icon when status is on"));
    icon_on_label.set_halign(Align::Start);
    icon_on_label.add_css_class("field-label");
    let icon_on_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_on_dropdown);

    let icon_off_label = Label::new(Some("Icon when status is off"));
    icon_off_label.set_halign(Align::Start);
    icon_off_label.add_css_class("field-label");
    let icon_off_dropdown = {
        let icons = icon_names.borrow();
        dropdown_with_icons(&state, icons.as_slice())
    };
    make_dropdown_shrinkable(&icon_off_dropdown);

    let interval_label = Label::new(Some("Status interval (ms)"));
    interval_label.set_halign(Align::Start);
    interval_label.add_css_class("field-label");
    let interval_spin = SpinButton::with_range(
        MIN_STATUS_INTERVAL_MS as f64,
        MAX_STATUS_INTERVAL_MS as f64,
        100.0,
    );
    interval_spin.set_hexpand(true);
    interval_spin.set_value(DEFAULT_STATUS_INTERVAL_MS as f64);

    let apply_button = Button::with_label("Apply");
    apply_button.add_css_class("action-button");
    apply_button.add_css_class("apply-button");
    apply_button.set_hexpand(true);
    apply_button.set_size_request(1, -1);
    let clear_button = Button::with_label("Delete");
    clear_button.set_tooltip_text(Some("Delete selected key configuration"));
    clear_button.add_css_class("action-button");
    clear_button.add_css_class("clear-button");
    clear_button.set_hexpand(true);
    clear_button.set_size_request(1, -1);
    let action_buttons_row = GtkBox::new(Orientation::Horizontal, 8);
    action_buttons_row.set_hexpand(true);
    action_buttons_row.set_homogeneous(true);
    action_buttons_row.set_margin_top(8);
    action_buttons_row.append(&apply_button);
    action_buttons_row.append(&clear_button);

    let status_line = Label::new(Some("Ready"));
    status_line.set_halign(Align::Start);
    status_line.add_css_class("status-label");

    editor.append(&selected_label);
    editor.append(&action_label);
    editor.append(&action_entry);
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
    editor.append(&action_buttons_row);

    editor_scroller.set_child(Some(&editor));
    inspector_panel.append(&editor_scroller);

    body.set_start_child(Some(&left_panel));
    body.set_end_child(Some(&inspector_panel));

    let header_bar = HeaderBar::new();
    header_bar.add_css_class("flat");
    header_bar.add_css_class("window-titlebar");
    let title_row = GtkBox::new(Orientation::Horizontal, 8);
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
    let empty_title = GtkBox::new(Orientation::Horizontal, 0);
    header_bar.set_title_widget(Some(&empty_title));
    header_bar.pack_start(&title_row);
    let status_bar = GtkBox::new(Orientation::Horizontal, 0);
    status_bar.add_css_class("status-bar");
    status_line.set_hexpand(true);
    status_line.set_halign(Align::Fill);
    status_line.set_xalign(0.0);
    status_bar.append(&status_line);
    root.append(&header_bar);
    root.append(&top_bar);
    root.append(&body);
    root.append(&status_bar);
    window.set_content(Some(&root));

    let widgets = EditorWidgets {
        config_path_entry,
        selected_label,
        action_entry,
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
        load_button.clone(),
        save_button.clone(),
        add_key_button.clone(),
        add_icon_button.clone(),
        apply_button.clone(),
        clear_button.clone(),
    );
}
