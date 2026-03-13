use super::super::*;
use adw::prelude::*;

fn selected_profile_name(widgets: &EditorWidgets) -> Option<String> {
    let selected = widgets.profile_dropdown.selected() as usize;
    widgets.profile_names.borrow().get(selected).cloned()
}

fn refresh_profile_selector(widgets: &EditorWidgets, selected_profile: &str) {
    let profiles = widgets.profile_names.borrow();
    let labels: Vec<String> = profiles
        .iter()
        .map(|profile| profile_display_name(profile))
        .collect();
    let names: Vec<&str> = labels.iter().map(String::as_str).collect();
    let list = gtk::StringList::new(&names);
    widgets.profile_dropdown.set_model(Some(&list));
    if let Some(selected_index) = profiles
        .iter()
        .position(|profile| profile == selected_profile)
    {
        widgets.profile_dropdown.set_selected(selected_index as u32);
    } else {
        widgets
            .profile_dropdown
            .set_selected(gtk::INVALID_LIST_POSITION);
    }
    widgets.profile_dropdown.set_sensitive(!profiles.is_empty());
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(trf(
            "Failed to remove '{path}': {err}",
            &[
                ("path", path.display().to_string()),
                ("err", err.to_string()),
            ],
        )),
    }
}

fn delete_profile_assets(profile: &str) -> Result<(), String> {
    let config_path = default_config_path_for_profile(profile);
    remove_if_exists(&config_path)
}

fn rename_profile_assets(from_profile: &str, to_profile: &str) -> Result<(), String> {
    if from_profile == to_profile {
        return Ok(());
    }

    let from_path = default_config_path_for_profile(from_profile);
    let to_path = default_config_path_for_profile(to_profile);
    if to_path.exists() {
        return Err(trf(
            "Profile '{profile}' already exists",
            &[("profile", to_profile.to_string())],
        ));
    }
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            trf(
                "Failed to create '{path}': {err}",
                &[
                    ("path", parent.display().to_string()),
                    ("err", err.to_string()),
                ],
            )
        })?;
    }
    fs::rename(&from_path, &to_path).map_err(|err| {
        trf(
            "Failed to rename profile config '{from}' -> '{to}': {err}",
            &[
                ("from", from_path.display().to_string()),
                ("to", to_path.display().to_string()),
                ("err", err.to_string()),
            ],
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn load_profile_into_ui(
    profile: &str,
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    selected_key: &Rc<Cell<usize>>,
    widgets: &EditorWidgets,
    icon_names: &Rc<RefCell<Vec<String>>>,
    clock_backgrounds: &Rc<RefCell<Vec<String>>>,
    prev_page_button: &Button,
    next_page_button: &Button,
    page_label: &Label,
    key_buttons: &[Button],
    key_pictures: &[Picture],
    editor_syncing: &Rc<Cell<bool>>,
) -> Result<PathBuf, String> {
    let path = default_config_path_for_profile(profile);
    let config = load_config(&path)?;
    let profile_changed = state.borrow().profile != profile;
    {
        let mut state = state.borrow_mut();
        update_state_profile_paths(&mut state, &path);
        state.config = config;
    }
    current_page.set(0);
    selected_key.set(0);

    let save_result = if profile_changed {
        save_current_profile(profile).map(|_| true)
    } else {
        save_current_profile_if_missing(profile)
    };
    if let Err(err) = save_result {
        eprintln!("{err}");
        return Err(err);
    }
    if let Err(err) = signal_daemon_reload() {
        eprintln!("{err}");
    }

    editor_syncing.set(true);
    refresh_icon_catalogs(state, icon_names, clock_backgrounds, widgets);

    clamp_page_and_selection(state, current_page, selected_key);
    refresh_page_controls(
        state,
        current_page,
        prev_page_button,
        next_page_button,
        page_label,
    );

    let selected = selected_key.get();
    refresh_selected_button_state(key_buttons, selected);
    let icons = icon_names.borrow();
    let backgrounds = clock_backgrounds.borrow();
    refresh_key_grid(
        state,
        key_buttons,
        key_pictures,
        current_page.get(),
        backgrounds.as_slice(),
    );
    populate_editor(
        state,
        current_page.get(),
        selected,
        widgets,
        icons.as_slice(),
        backgrounds.as_slice(),
    );
    editor_syncing.set(false);

    Ok(path)
}

pub(crate) fn wire_management_signals(
    window: &ApplicationWindow,
    ctx: &UiCtx,
    add_profile_button: &Button,
    remove_profile_button: &Button,
    rename_profile_button: &Button,
    add_key_button: &Button,
    add_icon_button: &Button,
) {
    let state = &ctx.state;
    let current_page = &ctx.current_page;
    let selected_key = &ctx.selected_key;
    let widgets = &ctx.widgets;
    let icon_names = &ctx.icon_names;
    let clock_backgrounds = &ctx.clock_backgrounds;
    let key_buttons = &ctx.key_buttons;
    let key_pictures = &ctx.key_pictures;
    let prev_page_button = &ctx.prev_page_button;
    let next_page_button = &ctx.next_page_button;
    let page_label = &ctx.page_label;
    let editor_syncing = &ctx.editor_syncing;
    {
        let state_for_select = state.clone();
        let current_page_for_select = current_page.clone();
        let selected_for_select = selected_key.clone();
        let widgets_for_select = widgets.clone();
        let icons_for_select = icon_names.clone();
        let backgrounds_for_select = clock_backgrounds.clone();
        let prev_for_select = prev_page_button.clone();
        let next_for_select = next_page_button.clone();
        let page_label_for_select = page_label.clone();
        let key_buttons_for_select = key_buttons.clone();
        let key_pictures_for_select = key_pictures.clone();
        let editor_syncing_for_select = editor_syncing.clone();
        widgets.profile_dropdown.connect_selected_notify(move |_| {
            if let Some(profile) = selected_profile_name(&widgets_for_select) {
                if let Err(err) = load_profile_into_ui(
                    &profile,
                    &state_for_select,
                    &current_page_for_select,
                    &selected_for_select,
                    &widgets_for_select,
                    &icons_for_select,
                    &backgrounds_for_select,
                    &prev_for_select,
                    &next_for_select,
                    &page_label_for_select,
                    &key_buttons_for_select,
                    &key_pictures_for_select,
                    &editor_syncing_for_select,
                ) {
                    widgets_for_select.status_label.set_text(&err);
                }
            }
        });
    }

    {
        let window_for_add_profile = window.clone();
        let state_for_add_profile = state.clone();
        let current_page_for_add_profile = current_page.clone();
        let selected_for_add_profile = selected_key.clone();
        let widgets_for_add_profile = widgets.clone();
        let icons_for_add_profile = icon_names.clone();
        let backgrounds_for_add_profile = clock_backgrounds.clone();
        let prev_for_add_profile = prev_page_button.clone();
        let next_for_add_profile = next_page_button.clone();
        let page_label_for_add_profile = page_label.clone();
        let key_buttons_for_add_profile = key_buttons.clone();
        let key_pictures_for_add_profile = key_pictures.clone();
        let editor_syncing_for_add_profile = editor_syncing.clone();
        let remove_button_for_add_profile = remove_profile_button.clone();
        let rename_button_for_add_profile = rename_profile_button.clone();

        add_profile_button.connect_clicked(move |_| {
            let dialog = gtk::Dialog::builder()
                .title(&tr("Add profile"))
                .transient_for(&window_for_add_profile)
                .modal(true)
                .build();
            dialog.add_button(&tr("Cancel"), gtk::ResponseType::Cancel);
            dialog.add_button(&tr("Add"), gtk::ResponseType::Accept);
            dialog.set_default_response(gtk::ResponseType::Accept);

            let content = dialog.content_area();
            content.set_spacing(8);
            content.set_margin_top(12);
            content.set_margin_bottom(12);
            content.set_margin_start(12);
            content.set_margin_end(12);

            let name_label = Label::new(Some(&tr("Profile name")));
            name_label.set_halign(Align::Start);
            name_label.add_css_class("field-label");
            let name_entry = Entry::new();
            name_entry.set_placeholder_text(Some(&tr("My Profile")));
            name_entry.set_activates_default(true);
            content.append(&name_label);
            content.append(&name_entry);

            let dialog_for_activate = dialog.clone();
            name_entry.connect_activate(move |_| {
                dialog_for_activate.response(gtk::ResponseType::Accept);
            });

            let state_for_response = state_for_add_profile.clone();
            let selected_for_response = selected_for_add_profile.clone();
            let widgets_for_response = widgets_for_add_profile.clone();
            let icons_for_response = icons_for_add_profile.clone();
            let backgrounds_for_response = backgrounds_for_add_profile.clone();
            let prev_for_response = prev_for_add_profile.clone();
            let next_for_response = next_for_add_profile.clone();
            let page_label_for_response = page_label_for_add_profile.clone();
            let current_page_for_response = current_page_for_add_profile.clone();
            let key_buttons_for_response = key_buttons_for_add_profile.clone();
            let key_pictures_for_response = key_pictures_for_add_profile.clone();
            let editor_syncing_for_response = editor_syncing_for_add_profile.clone();
            let remove_button_for_response = remove_button_for_add_profile.clone();
            let rename_button_for_response = rename_button_for_add_profile.clone();
            let name_entry_for_response = name_entry.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    let Some(profile) =
                        profile_slug_from_input(name_entry_for_response.text().as_str())
                    else {
                        widgets_for_response
                            .status_label
                            .set_text(&tr("Profile name must contain letters or numbers"));
                        dialog.hide();
                        return;
                    };

                    let is_new_profile = {
                        let names = widgets_for_response.profile_names.borrow();
                        !names.iter().any(|name| name == &profile)
                    };

                    if is_new_profile {
                        let path = default_config_path_for_profile(&profile);
                        if let Err(err) = save_config(&path, &Config::default()) {
                            widgets_for_response.status_label.set_text(&err);
                            dialog.hide();
                            return;
                        }
                    }

                    {
                        let mut names = widgets_for_response.profile_names.borrow_mut();
                        if !names.iter().any(|name| name == &profile) {
                            names.push(profile.clone());
                            names.sort_unstable();
                            names.dedup();
                        }
                    }
                    refresh_profile_selector(&widgets_for_response, &profile);
                    remove_button_for_response.set_sensitive(true);
                    rename_button_for_response.set_sensitive(true);

                    match load_profile_into_ui(
                        &profile,
                        &state_for_response,
                        &current_page_for_response,
                        &selected_for_response,
                        &widgets_for_response,
                        &icons_for_response,
                        &backgrounds_for_response,
                        &prev_for_response,
                        &next_for_response,
                        &page_label_for_response,
                        &key_buttons_for_response,
                        &key_pictures_for_response,
                        &editor_syncing_for_response,
                    ) {
                        Ok(_) => {
                            if is_new_profile {
                                widgets_for_response.status_label.set_text(&trf(
                                    "Created and loaded profile '{profile}'",
                                    &[("profile", profile_display_name(&profile))],
                                ));
                            } else {
                                widgets_for_response.status_label.set_text(&trf(
                                    "Loaded profile '{profile}'",
                                    &[("profile", profile_display_name(&profile))],
                                ));
                            }
                        }
                        Err(err) => widgets_for_response.status_label.set_text(&err),
                    }
                }
                dialog.hide();
            });
            dialog.show();
        });
    }

    {
        let window_for_rename_profile = window.clone();
        let state_for_rename_profile = state.clone();
        let current_page_for_rename_profile = current_page.clone();
        let selected_for_rename_profile = selected_key.clone();
        let widgets_for_rename_profile = widgets.clone();
        let icons_for_rename_profile = icon_names.clone();
        let backgrounds_for_rename_profile = clock_backgrounds.clone();
        let prev_for_rename_profile = prev_page_button.clone();
        let next_for_rename_profile = next_page_button.clone();
        let page_label_for_rename_profile = page_label.clone();
        let key_buttons_for_rename_profile = key_buttons.clone();
        let key_pictures_for_rename_profile = key_pictures.clone();
        let editor_syncing_for_rename_profile = editor_syncing.clone();
        let remove_button_for_rename_profile = remove_profile_button.clone();
        let rename_button_for_rename_profile = rename_profile_button.clone();

        rename_profile_button.connect_clicked(move |_| {
            let Some(current_profile) = selected_profile_name(&widgets_for_rename_profile) else {
                widgets_for_rename_profile
                    .status_label
                    .set_text(&tr("No profile selected"));
                return;
            };

            let dialog = gtk::Dialog::builder()
                .title(&tr("Rename profile"))
                .transient_for(&window_for_rename_profile)
                .modal(true)
                .build();
            dialog.add_button(&tr("Cancel"), gtk::ResponseType::Cancel);
            dialog.add_button(&tr("Rename"), gtk::ResponseType::Accept);
            dialog.set_default_response(gtk::ResponseType::Accept);

            let content = dialog.content_area();
            content.set_spacing(8);
            content.set_margin_top(12);
            content.set_margin_bottom(12);
            content.set_margin_start(12);
            content.set_margin_end(12);

            let name_label = Label::new(Some(&tr("New profile name")));
            name_label.set_halign(Align::Start);
            name_label.add_css_class("field-label");
            let name_entry = Entry::new();
            name_entry.set_text(&profile_display_name(&current_profile));
            name_entry.set_activates_default(true);
            content.append(&name_label);
            content.append(&name_entry);

            let dialog_for_activate = dialog.clone();
            name_entry.connect_activate(move |_| {
                dialog_for_activate.response(gtk::ResponseType::Accept);
            });

            let state_for_response = state_for_rename_profile.clone();
            let selected_for_response = selected_for_rename_profile.clone();
            let widgets_for_response = widgets_for_rename_profile.clone();
            let icons_for_response = icons_for_rename_profile.clone();
            let backgrounds_for_response = backgrounds_for_rename_profile.clone();
            let prev_for_response = prev_for_rename_profile.clone();
            let next_for_response = next_for_rename_profile.clone();
            let page_label_for_response = page_label_for_rename_profile.clone();
            let current_page_for_response = current_page_for_rename_profile.clone();
            let key_buttons_for_response = key_buttons_for_rename_profile.clone();
            let key_pictures_for_response = key_pictures_for_rename_profile.clone();
            let editor_syncing_for_response = editor_syncing_for_rename_profile.clone();
            let remove_button_for_response = remove_button_for_rename_profile.clone();
            let rename_button_for_response = rename_button_for_rename_profile.clone();
            let current_profile_for_response = current_profile.clone();
            let name_entry_for_response = name_entry.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    let Some(new_profile) =
                        profile_slug_from_input(name_entry_for_response.text().as_str())
                    else {
                        widgets_for_response
                            .status_label
                            .set_text(&tr("Profile name must contain letters or numbers"));
                        dialog.hide();
                        return;
                    };

                    if new_profile != current_profile_for_response {
                        let exists = widgets_for_response
                            .profile_names
                            .borrow()
                            .iter()
                            .any(|name| name == &new_profile);
                        if exists {
                            widgets_for_response.status_label.set_text(&trf(
                                "Profile '{profile}' already exists",
                                &[("profile", new_profile.clone())],
                            ));
                            dialog.hide();
                            return;
                        }

                        if let Err(err) =
                            rename_profile_assets(&current_profile_for_response, &new_profile)
                        {
                            widgets_for_response.status_label.set_text(&err);
                            dialog.hide();
                            return;
                        }
                    }

                    {
                        let mut names = widgets_for_response.profile_names.borrow_mut();
                        if let Some(index) = names
                            .iter()
                            .position(|n| n == &current_profile_for_response)
                        {
                            names[index] = new_profile.clone();
                        }
                        names.sort_unstable();
                        names.dedup();
                    }
                    refresh_profile_selector(&widgets_for_response, &new_profile);
                    let has_profiles = !widgets_for_response.profile_names.borrow().is_empty();
                    remove_button_for_response.set_sensitive(has_profiles);
                    rename_button_for_response.set_sensitive(has_profiles);

                    match load_profile_into_ui(
                        &new_profile,
                        &state_for_response,
                        &current_page_for_response,
                        &selected_for_response,
                        &widgets_for_response,
                        &icons_for_response,
                        &backgrounds_for_response,
                        &prev_for_response,
                        &next_for_response,
                        &page_label_for_response,
                        &key_buttons_for_response,
                        &key_pictures_for_response,
                        &editor_syncing_for_response,
                    ) {
                        Ok(_) => widgets_for_response.status_label.set_text(&trf(
                            "Renamed profile '{from}' -> '{to}'",
                            &[
                                ("from", profile_display_name(&current_profile_for_response)),
                                ("to", profile_display_name(&new_profile)),
                            ],
                        )),
                        Err(err) => widgets_for_response.status_label.set_text(&err),
                    }
                }
                dialog.hide();
            });
            dialog.show();
        });
    }

    {
        let window_for_remove_profile = window.clone();
        let state_for_remove_profile = state.clone();
        let current_page_for_remove_profile = current_page.clone();
        let selected_for_remove_profile = selected_key.clone();
        let widgets_for_remove_profile = widgets.clone();
        let icons_for_remove_profile = icon_names.clone();
        let backgrounds_for_remove_profile = clock_backgrounds.clone();
        let prev_for_remove_profile = prev_page_button.clone();
        let next_for_remove_profile = next_page_button.clone();
        let page_label_for_remove_profile = page_label.clone();
        let key_buttons_for_remove_profile = key_buttons.clone();
        let key_pictures_for_remove_profile = key_pictures.clone();
        let editor_syncing_for_remove_profile = editor_syncing.clone();
        let remove_button_for_remove_profile = remove_profile_button.clone();
        let rename_button_for_remove_profile = rename_profile_button.clone();

        remove_profile_button.connect_clicked(move |_| {
            let Some(profile) = selected_profile_name(&widgets_for_remove_profile) else {
                widgets_for_remove_profile
                    .status_label
                    .set_text(&tr("No profile selected"));
                return;
            };

            let dialog = gtk::Dialog::builder()
                .title(&tr("Remove profile"))
                .transient_for(&window_for_remove_profile)
                .modal(true)
                .build();
            dialog.add_button(&tr("Cancel"), gtk::ResponseType::Cancel);
            dialog.add_button(&tr("Remove"), gtk::ResponseType::Accept);
            dialog.set_default_response(gtk::ResponseType::Cancel);

            let content = dialog.content_area();
            content.set_spacing(8);
            content.set_margin_top(12);
            content.set_margin_bottom(12);
            content.set_margin_start(12);
            content.set_margin_end(12);
            let warning = Label::new(Some(&trf(
                "Delete profile '{profile}' config? Shared icons are kept.",
                &[("profile", profile.clone())],
            )));
            warning.set_wrap(true);
            warning.set_xalign(0.0);
            content.append(&warning);

            let state_for_response = state_for_remove_profile.clone();
            let selected_for_response = selected_for_remove_profile.clone();
            let widgets_for_response = widgets_for_remove_profile.clone();
            let icons_for_response = icons_for_remove_profile.clone();
            let backgrounds_for_response = backgrounds_for_remove_profile.clone();
            let prev_for_response = prev_for_remove_profile.clone();
            let next_for_response = next_for_remove_profile.clone();
            let page_label_for_response = page_label_for_remove_profile.clone();
            let current_page_for_response = current_page_for_remove_profile.clone();
            let key_buttons_for_response = key_buttons_for_remove_profile.clone();
            let key_pictures_for_response = key_pictures_for_remove_profile.clone();
            let editor_syncing_for_response = editor_syncing_for_remove_profile.clone();
            let remove_button_for_response = remove_button_for_remove_profile.clone();
            let rename_button_for_response = rename_button_for_remove_profile.clone();
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    let Some(profile) = selected_profile_name(&widgets_for_response) else {
                        widgets_for_response
                            .status_label
                            .set_text(&tr("No profile selected"));
                        dialog.hide();
                        return;
                    };

                    if let Err(err) = delete_profile_assets(&profile) {
                        widgets_for_response.status_label.set_text(&err);
                        dialog.hide();
                        return;
                    }

                    let next_profile = {
                        let mut names = widgets_for_response.profile_names.borrow_mut();
                        names.retain(|name| name != &profile);
                        if names.is_empty() {
                            BLANK_PROFILE.to_string()
                        } else if let Some(default_profile) =
                            names.iter().find(|name| name.as_str() == DEFAULT_PROFILE)
                        {
                            default_profile.clone()
                        } else {
                            names[0].clone()
                        }
                    };

                    let next_path = default_config_path_for_profile(&next_profile);
                    if !next_path.is_file() {
                        let template = if next_profile == BLANK_PROFILE {
                            streamrs::config::streamrs_schema::blank_profile_config()
                        } else {
                            Config::default()
                        };
                        if let Err(err) = save_config(&next_path, &template) {
                            widgets_for_response.status_label.set_text(&err);
                            dialog.hide();
                            return;
                        }
                    }

                    refresh_profile_selector(&widgets_for_response, &next_profile);
                    let has_profiles = !widgets_for_response.profile_names.borrow().is_empty();
                    remove_button_for_response.set_sensitive(has_profiles);
                    rename_button_for_response.set_sensitive(has_profiles);
                    match load_profile_into_ui(
                        &next_profile,
                        &state_for_response,
                        &current_page_for_response,
                        &selected_for_response,
                        &widgets_for_response,
                        &icons_for_response,
                        &backgrounds_for_response,
                        &prev_for_response,
                        &next_for_response,
                        &page_label_for_response,
                        &key_buttons_for_response,
                        &key_pictures_for_response,
                        &editor_syncing_for_response,
                    ) {
                        Ok(_) => widgets_for_response.status_label.set_text(&trf(
                            "Removed profile '{profile}', active profile is '{next_profile}'",
                            &[("profile", profile), ("next_profile", next_profile)],
                        )),
                        Err(err) => widgets_for_response.status_label.set_text(&err),
                    }
                }
                dialog.hide();
            });
            dialog.show();
        });
    }

    {
        let state_for_add_key = state.clone();
        let current_page_for_add_key = current_page.clone();
        let selected_for_add_key = selected_key.clone();
        let widgets_for_add_key = widgets.clone();
        let icons_for_add_key = icon_names.clone();
        let backgrounds_for_add_key = clock_backgrounds.clone();
        let prev_for_add_key = prev_page_button.clone();
        let next_for_add_key = next_page_button.clone();
        let page_label_for_add_key = page_label.clone();
        let key_buttons_for_add_key = key_buttons.clone();
        let key_pictures_for_add_key = key_pictures.clone();
        let editor_syncing_for_add_key = editor_syncing.clone();

        add_key_button.connect_clicked(move |_| {
            let (new_key_index, target_page, target_slot) = {
                let mut state = state_for_add_key.borrow_mut();
                normalize_config(&mut state.config);
                state.config.keys.push(KeyBinding::default());
                let new_key_index = state.config.keys.len().saturating_sub(1);
                let (target_page, target_slot) =
                    locate_key_slot(&state.config, new_key_index).unwrap_or((0, 0));
                (new_key_index, target_page, target_slot)
            };

            current_page_for_add_key.set(target_page);
            selected_for_add_key.set(target_slot);
            clamp_page_and_selection(
                &state_for_add_key,
                &current_page_for_add_key,
                &selected_for_add_key,
            );
            refresh_page_controls(
                &state_for_add_key,
                &current_page_for_add_key,
                &prev_for_add_key,
                &next_for_add_key,
                &page_label_for_add_key,
            );

            let selected = selected_for_add_key.get();
            refresh_selected_button_state(&key_buttons_for_add_key, selected);
            let icons = icons_for_add_key.borrow();
            let backgrounds = backgrounds_for_add_key.borrow();
            refresh_key_grid(
                &state_for_add_key,
                &key_buttons_for_add_key,
                &key_pictures_for_add_key,
                current_page_for_add_key.get(),
                backgrounds.as_slice(),
            );
            populate_editor_guarded(
                &state_for_add_key,
                current_page_for_add_key.get(),
                selected,
                &widgets_for_add_key,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_add_key,
            );
            widgets_for_add_key.status_label.set_text(&trf(
                "Added button {index}",
                &[("index", (new_key_index + 1).to_string())],
            ));
        });
    }

    {
        let state_for_add_icon = state.clone();
        let current_page_for_add_icon = current_page.clone();
        let selected_for_add_icon = selected_key.clone();
        let widgets_for_add_icon = widgets.clone();
        let icons_for_add_icon = icon_names.clone();
        let backgrounds_for_add_icon = clock_backgrounds.clone();
        let prev_for_add_icon = prev_page_button.clone();
        let next_for_add_icon = next_page_button.clone();
        let page_label_for_add_icon = page_label.clone();
        let key_buttons_for_add_icon = key_buttons.clone();
        let key_pictures_for_add_icon = key_pictures.clone();
        let window_for_add_icon = window.clone();
        let editor_syncing_for_add_icon = editor_syncing.clone();

        add_icon_button.connect_clicked(move |_| {
            let dialog = gtk::FileChooserNative::builder()
                .title(&tr("Add icon"))
                .transient_for(&window_for_add_icon)
                .modal(true)
                .action(gtk::FileChooserAction::Open)
                .accept_label(&tr("Add"))
                .cancel_label(&tr("Cancel"))
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&tr("Images")));
            filter.add_pattern("*.png");
            filter.add_pattern("*.jpg");
            filter.add_pattern("*.jpeg");
            filter.add_pattern("*.gif");
            filter.add_pattern("*.webp");
            filter.add_pattern("*.svg");
            dialog.add_filter(&filter);

            let state_for_response = state_for_add_icon.clone();
            let selected_for_response = selected_for_add_icon.clone();
            let widgets_for_response = widgets_for_add_icon.clone();
            let icons_for_response = icons_for_add_icon.clone();
            let backgrounds_for_response = backgrounds_for_add_icon.clone();
            let prev_for_response = prev_for_add_icon.clone();
            let next_for_response = next_for_add_icon.clone();
            let page_label_for_response = page_label_for_add_icon.clone();
            let current_page_for_response = current_page_for_add_icon.clone();
            let key_buttons_for_response = key_buttons_for_add_icon.clone();
            let key_pictures_for_response = key_pictures_for_add_icon.clone();
            let editor_syncing_for_response = editor_syncing_for_add_icon.clone();
            dialog.connect_response(move |chooser, response| {
                if response == gtk::ResponseType::Accept {
                    let picked = chooser.file().and_then(|file| file.path());
                    if let Some(source_path) = picked {
                        let writable_image_dir =
                            state_for_response.borrow().writable_image_dir.clone();
                        match copy_icon_into_profile(&source_path, &writable_image_dir) {
                            Ok(icon_name) => {
                                editor_syncing_for_response.set(true);
                                refresh_icon_catalogs(
                                    &state_for_response,
                                    &icons_for_response,
                                    &backgrounds_for_response,
                                    &widgets_for_response,
                                );
                                clamp_page_and_selection(
                                    &state_for_response,
                                    &current_page_for_response,
                                    &selected_for_response,
                                );
                                refresh_page_controls(
                                    &state_for_response,
                                    &current_page_for_response,
                                    &prev_for_response,
                                    &next_for_response,
                                    &page_label_for_response,
                                );
                                let selected = selected_for_response.get();
                                let icons = icons_for_response.borrow();
                                let backgrounds = backgrounds_for_response.borrow();
                                let page = current_page_for_response.get();
                                widgets_for_response.icon_kind_dropdown.set_selected(1);
                                set_editor_mode_visibility(
                                    &widgets_for_response,
                                    EditorMode::Regular,
                                );
                                set_dropdown_icon(
                                    &widgets_for_response.icon_dropdown,
                                    icons.as_slice(),
                                    &icon_name,
                                );
                                editor_syncing_for_response.set(false);
                                apply_editor_to_selected_key(
                                    &state_for_response,
                                    page,
                                    selected,
                                    &widgets_for_response,
                                    icons.as_slice(),
                                    backgrounds.as_slice(),
                                );
                                refresh_key_grid(
                                    &state_for_response,
                                    &key_buttons_for_response,
                                    &key_pictures_for_response,
                                    page,
                                    backgrounds.as_slice(),
                                );
                                populate_editor_guarded(
                                    &state_for_response,
                                    page,
                                    selected,
                                    &widgets_for_response,
                                    icons.as_slice(),
                                    backgrounds.as_slice(),
                                    &editor_syncing_for_response,
                                );
                                widgets_for_response.status_label.set_text(&trf(
                                    "Added and selected icon '{icon}' in '{dir}'",
                                    &[
                                        ("icon", icon_name),
                                        ("dir", writable_image_dir.display().to_string()),
                                    ],
                                ));
                            }
                            Err(err) => widgets_for_response.status_label.set_text(&err),
                        }
                    }
                }
                chooser.hide();
            });
            dialog.show();
        });
    }
}
