use super::*;

pub(crate) fn copy_icon_into_profile(source_path: &Path, target_dir: &Path) -> Result<String, String> {
    copy_supported_image_into_dir(source_path, target_dir)
}

pub(crate) fn discover_icons(image_dirs: &[PathBuf]) -> Vec<String> {
    discover_icons_generic(image_dirs, &[NAV_PREVIOUS_ICON, NAV_NEXT_ICON], "blank.png")
}

pub(crate) fn discover_clock_backgrounds(image_dirs: &[PathBuf]) -> Vec<String> {
    discover_png_backgrounds_with_prefix(image_dirs, "blank", CLOCK_BACKGROUND_ICON)
}

pub(crate) fn configure_icon_dropdown(dropdown: &DropDown, state: &Rc<RefCell<AppState>>) {
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
    if dropdown.find_property("search-match-mode").is_some() {
        dropdown.set_property("search-match-mode", gtk::StringFilterMatchMode::Substring);
    }
    let expression = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<&gtk::Expression>,
        "string",
    );
    dropdown.set_expression(Some(expression));
}

pub(crate) fn dropdown_with_icons(state: &Rc<RefCell<AppState>>, icon_names: &[String]) -> DropDown {
    let names: Vec<&str> = icon_names.iter().map(String::as_str).collect();
    let dropdown = DropDown::from_strings(&names);
    configure_icon_dropdown(&dropdown, state);
    dropdown
}

pub(crate) fn dropdown_set_options(dropdown: &DropDown, icon_names: &[String]) {
    let names: Vec<&str> = icon_names.iter().map(String::as_str).collect();
    let list = gtk::StringList::new(&names);
    dropdown.set_model(Some(&list));
}

pub(crate) fn make_dropdown_shrinkable(dropdown: &DropDown) {
    dropdown.set_hexpand(true);
    dropdown.set_size_request(1, -1);
}

pub(crate) fn refresh_icon_catalogs(
    state: &Rc<RefCell<AppState>>,
    icon_names: &Rc<RefCell<Vec<String>>>,
    clock_backgrounds: &Rc<RefCell<Vec<String>>>,
    widgets: &EditorWidgets,
) {
    let catalog_dirs = state.borrow().image_dirs.clone();
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

pub(crate) fn dropdown_selected_icon(dropdown: &DropDown, icon_names: &[String]) -> String {
    let index = dropdown.selected() as usize;
    icon_names
        .get(index)
        .cloned()
        .unwrap_or_else(default_icon_name)
}

pub(crate) fn set_dropdown_icon(dropdown: &DropDown, icon_names: &[String], icon_name: &str) {
    if let Some(index) = icon_names.iter().position(|candidate| candidate == icon_name) {
        dropdown.set_selected(index as u32);
    } else {
        dropdown.set_selected(0);
    }
}

pub(crate) fn update_picture_file(picture: &Picture, path: Option<&Path>) {
    if let Some(path) = path {
        let file = gtk::gio::File::for_path(path);
        picture.set_file(Some(&file));
    } else {
        picture.set_file(None::<&gtk::gio::File>);
    }
}

pub(crate) fn find_icon_file(image_dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    image_dirs
        .iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

pub(crate) fn key_clock_background_name<'a>(key: &'a KeyBinding, defaults: &'a [String]) -> &'a str {
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

pub(crate) fn set_picture_icon(
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

pub(crate) fn refresh_selected_button_state(buttons: &[Button], selected_key: usize) {
    for (index, button) in buttons.iter().enumerate() {
        if index == selected_key {
            button.add_css_class("key-selected");
        } else {
            button.remove_css_class("key-selected");
        }
    }
}
