fn load_config(path: &Path) -> Result<Config, String> {
    let profile = profile_from_config_path(path);
    for candidate in config_load_candidates(&profile, path) {
        if !candidate.is_file() {
            continue;
        }

        let raw = fs::read_to_string(&candidate)
            .map_err(|err| format!("Failed to read config '{}': {err}", candidate.display()))?;
        let mut config: Config = toml::from_str(&raw)
            .map_err(|err| format!("Failed to parse config '{}': {err}", candidate.display()))?;
        normalize_config(&mut config);
        return Ok(config);
    }

    Ok(Config::default())
}

fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create config directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    let output = toml::to_string_pretty(config)
        .map_err(|err| format!("Failed to serialize config '{}': {err}", path.display()))?;
    fs::write(path, output)
        .map_err(|err| format!("Failed to write config '{}': {err}", path.display()))?;
    Ok(())
}

fn is_supported_icon_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg"
    )
}

fn copy_icon_into_profile(source_path: &Path, target_dir: &Path) -> Result<String, String> {
    if !source_path.is_file() {
        return Err(format!(
            "Selected path '{}' is not a file",
            source_path.display()
        ));
    }
    if !is_supported_icon_extension(source_path) {
        return Err(format!(
            "Unsupported icon type for '{}'",
            source_path.display()
        ));
    }

    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Icon file name is not valid UTF-8".to_string())?
        .to_string();
    fs::create_dir_all(target_dir).map_err(|err| {
        format!(
            "Failed to create icon directory '{}': {err}",
            target_dir.display()
        )
    })?;

    let destination = target_dir.join(&file_name);
    if destination == source_path {
        return Ok(file_name);
    }

    fs::copy(source_path, &destination).map_err(|err| {
        format!(
            "Failed to copy icon '{}' to '{}': {err}",
            source_path.display(),
            destination.display()
        )
    })?;
    Ok(file_name)
}

fn discover_icons(image_dirs: &[PathBuf]) -> Vec<String> {
    let mut icons = Vec::new();

    for image_dir in image_dirs {
        if let Ok(entries) = fs::read_dir(image_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                if !is_supported_icon_extension(&path) {
                    continue;
                }

                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    if name == NAV_PREVIOUS_ICON || name == NAV_NEXT_ICON {
                        continue;
                    }
                    icons.push(name.to_string());
                }
            }
        }
    }

    icons.sort_by_key(|name| name.to_ascii_lowercase());
    icons.dedup();

    if let Some(blank_index) = icons.iter().position(|name| name == "blank.png") {
        if blank_index != 0 {
            let blank = icons.remove(blank_index);
            icons.insert(0, blank);
        }
    } else {
        icons.insert(0, "blank.png".to_string());
    }

    icons
}

fn discover_clock_backgrounds(image_dirs: &[PathBuf]) -> Vec<String> {
    let mut backgrounds = Vec::new();

    for image_dir in image_dirs {
        if let Ok(entries) = fs::read_dir(image_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if extension != "png" {
                    continue;
                }

                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if !name.starts_with("blank") {
                    continue;
                }

                backgrounds.push(name.to_string());
            }
        }
    }

    backgrounds.sort_by_key(|name| name.to_ascii_lowercase());
    backgrounds.dedup();
    if let Some(index) = backgrounds.iter().position(|name| name == CLOCK_BACKGROUND_ICON) {
        if index != 0 {
            let blank = backgrounds.remove(index);
            backgrounds.insert(0, blank);
        }
    } else {
        backgrounds.insert(0, CLOCK_BACKGROUND_ICON.to_string());
    }

    backgrounds
}

fn configure_icon_dropdown(dropdown: &DropDown, state: &Rc<RefCell<AppState>>) {
    let state_for_bind = state.clone();
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        let icon = Picture::new();
        icon.set_size_request(24, 24);
        icon.set_keep_aspect_ratio(true);
        icon.set_can_shrink(true);
        icon.add_css_class("dropdown-icon");

        let label = Label::new(None);
        label.set_halign(Align::Start);
        label.set_hexpand(true);
        label.set_xalign(0.0);

        row.append(&icon);
        row.append(&label);
        list_item.set_child(Some(&row));
    });
    factory.connect_bind(move |_, list_item| {
        let Some(item) = list_item.item() else {
            return;
        };
        let Ok(item) = item.downcast::<gtk::StringObject>() else {
            return;
        };
        let name = item.string().to_string();

        let Some(row_widget) = list_item.child() else {
            return;
        };
        let Ok(row) = row_widget.downcast::<GtkBox>() else {
            return;
        };
        let Some(icon_widget) = row.first_child() else {
            return;
        };
        let Some(label_widget) = row.last_child() else {
            return;
        };
        let Ok(icon) = icon_widget.downcast::<Picture>() else {
            return;
        };
        let Ok(label) = label_widget.downcast::<Label>() else {
            return;
        };

        label.set_text(&name);

        let image_dirs = state_for_bind.borrow().image_dirs.clone();
        let preview_path = if icon_is_clock(&name) {
            render_clock_icon_png(&image_dirs, Some(CLOCK_BACKGROUND_ICON))
        } else {
            render_regular_icon_png(&image_dirs, &name).or_else(|| find_icon_file(&image_dirs, &name))
        };
        update_picture_file(&icon, preview_path.as_deref());
    });

    dropdown.set_factory(Some(&factory));
    dropdown.set_list_factory(Some(&factory));
    dropdown.set_enable_search(true);
    let expression = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<&gtk::Expression>,
        "string",
    );
    dropdown.set_expression(Some(expression));
}

fn dropdown_with_icons(state: &Rc<RefCell<AppState>>, icon_names: &[String]) -> DropDown {
    let names: Vec<&str> = icon_names.iter().map(String::as_str).collect();
    let dropdown = DropDown::from_strings(&names);
    configure_icon_dropdown(&dropdown, state);
    dropdown
}

fn dropdown_set_options(dropdown: &DropDown, icon_names: &[String]) {
    let names: Vec<&str> = icon_names.iter().map(String::as_str).collect();
    let list = gtk::StringList::new(&names);
    dropdown.set_model(Some(&list));
}

fn make_dropdown_shrinkable(dropdown: &DropDown) {
    dropdown.set_hexpand(true);
    dropdown.set_size_request(1, -1);
}

fn refresh_icon_catalogs(
    state: &Rc<RefCell<AppState>>,
    icon_names: &Rc<RefCell<Vec<String>>>,
    clock_backgrounds: &Rc<RefCell<Vec<String>>>,
    widgets: &EditorWidgets,
) {
    let catalog_dirs = vec![state.borrow().writable_image_dir.clone()];
    *icon_names.borrow_mut() = discover_icons(&catalog_dirs);
    *clock_backgrounds.borrow_mut() = discover_clock_backgrounds(&catalog_dirs);

    {
        let icons = icon_names.borrow();
        dropdown_set_options(&widgets.icon_dropdown, icons.as_slice());
        dropdown_set_options(&widgets.icon_on_dropdown, icons.as_slice());
        dropdown_set_options(&widgets.icon_off_dropdown, icons.as_slice());
    }
    {
        let backgrounds = clock_backgrounds.borrow();
        dropdown_set_options(&widgets.clock_background_dropdown, backgrounds.as_slice());
    }
}

fn dropdown_selected_icon(dropdown: &DropDown, icon_names: &[String]) -> String {
    let index = dropdown.selected() as usize;
    icon_names
        .get(index)
        .cloned()
        .unwrap_or_else(default_icon_name)
}

fn set_dropdown_icon(dropdown: &DropDown, icon_names: &[String], icon_name: &str) {
    if let Some(index) = icon_names.iter().position(|candidate| candidate == icon_name) {
        dropdown.set_selected(index as u32);
    } else {
        dropdown.set_selected(0);
    }
}

fn update_picture_file(picture: &Picture, path: Option<&Path>) {
    if let Some(path) = path {
        let file = gtk::gio::File::for_path(path);
        picture.set_file(Some(&file));
    } else {
        picture.set_file(None::<&gtk::gio::File>);
    }
}

fn find_icon_file(image_dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    image_dirs
        .iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn key_clock_background_name<'a>(key: &'a KeyBinding, defaults: &'a [String]) -> &'a str {
    if let Some(background) = key.clock_background.as_deref()
        && defaults.iter().any(|name| name == background)
    {
        return background;
    }
    defaults
        .first()
        .map(String::as_str)
        .unwrap_or(CLOCK_BACKGROUND_ICON)
}

fn set_picture_icon(
    picture: &Picture,
    image_dirs: &[PathBuf],
    key: &KeyBinding,
    clock_backgrounds: &[String],
) {
    let rounded = if icon_is_clock(&key.icon) {
        let background = key_clock_background_name(key, clock_backgrounds);
        render_clock_icon_png(image_dirs, Some(background))
    } else {
        render_regular_icon_png(image_dirs, &key.icon)
    };

    if let Some(rounded_path) = rounded {
        update_picture_file(picture, Some(&rounded_path));
        picture.set_tooltip_text(Some(&key.icon));
        return;
    }

    if let Some(fallback) = find_icon_file(image_dirs, "blank.png") {
        update_picture_file(picture, Some(&fallback));
    } else {
        update_picture_file(picture, None);
    }
    picture.set_tooltip_text(Some(&key.icon));
}

fn refresh_selected_button_state(buttons: &[Button], selected_key: usize) {
    for (index, button) in buttons.iter().enumerate() {
        if index == selected_key {
            button.add_css_class("key-selected");
        } else {
            button.remove_css_class("key-selected");
        }
    }
}

fn install_css() {
    let css = r#"
.streamrs-root { padding: 12px; }
headerbar.window-titlebar {
    background: transparent;
    background-image: none;
    box-shadow: none;
    border: none;
}
.config-bar {
    padding: 10px 12px;
    border-radius: 12px;
    background: alpha(@headerbar_bg_color, 0.45);
}
.deck-card, .inspector-card {
    border-radius: 16px;
    background: alpha(@headerbar_bg_color, 0.20);
    padding: 14px;
}
.section-title { font-weight: 700; font-size: 1.04rem; margin-bottom: 8px; }
.header-title-label { font-weight: 700; }
.field-label { font-weight: 600; opacity: 0.92; margin-top: 4px; }
.status-label { opacity: 0.85; }
.status-bar {
    margin-top: 8px;
    padding: 8px 12px;
    border-radius: 10px;
    background: alpha(@headerbar_bg_color, 0.20);
}
.close-button { min-width: 34px; min-height: 34px; }
.key-button {
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
}
.key-button:hover {
    background: alpha(@accent_color, 0.08);
}
.key-selected {
    outline: 2px solid @accent_color;
    outline-offset: 0;
    border-radius: 18px;
}
.key-drop-swap {
    outline: 2px solid alpha(@accent_color, 0.95);
    outline-offset: 0;
    border-radius: 18px;
}
.key-drop-before {
    box-shadow: inset 3px 0 0 0 alpha(@accent_color, 0.95);
    border-radius: 18px;
}
.key-drop-after {
    box-shadow: inset -3px 0 0 0 alpha(@accent_color, 0.95);
    border-radius: 18px;
}
.deck-image {
    border-radius: 26px;
}
.icon-preview {
    border-radius: 14px;
    background: alpha(@view_bg_color, 0.35);
    padding: 6px;
}
.key-icon {
    border-radius: 14px;
}
.dropdown-icon {
    border-radius: 6px;
}
.icon-add-button {
    min-width: 28px;
    min-height: 28px;
    padding: 0;
}
.action-button {
    min-height: 38px;
    min-width: 0;
    border-radius: 12px;
    font-weight: 700;
    padding: 0 14px;
}
.apply-button {
    background: alpha(@accent_color, 0.80);
    color: @accent_fg_color;
}
.clear-button {
    background: alpha(#864a66, 0.45);
    color: #ffc1d5;
}
"#;

    let provider = CssProvider::new();
    provider.load_from_data(css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
