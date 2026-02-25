use super::super::*;
use adw::prelude::*;

pub(crate) fn wire_management_signals(
    window: &ApplicationWindow,
    ctx: &UiCtx,
    load_button: &Button,
    save_button: &Button,
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
        let state_for_load = state.clone();
        let current_page_for_load = current_page.clone();
        let selected_for_load = selected_key.clone();
        let widgets_for_load = widgets.clone();
        let icons_for_load = icon_names.clone();
        let backgrounds_for_load = clock_backgrounds.clone();
        let prev_for_load = prev_page_button.clone();
        let next_for_load = next_page_button.clone();
        let page_label_for_load = page_label.clone();
        let key_buttons_for_load = key_buttons.clone();
        let key_pictures_for_load = key_pictures.clone();
        let editor_syncing_for_load = editor_syncing.clone();

        load_button.connect_clicked(move |_| {
            let path = PathBuf::from(widgets_for_load.config_path_entry.text().as_str());
            match load_config(&path) {
                Ok(config) => {
                    {
                        let mut state = state_for_load.borrow_mut();
                        update_state_profile_paths(&mut state, &path);
                        state.config = config;
                    }
                    refresh_icon_catalogs(
                        &state_for_load,
                        &icons_for_load,
                        &backgrounds_for_load,
                        &widgets_for_load,
                    );

                    clamp_page_and_selection(
                        &state_for_load,
                        &current_page_for_load,
                        &selected_for_load,
                    );
                    refresh_page_controls(
                        &state_for_load,
                        &current_page_for_load,
                        &prev_for_load,
                        &next_for_load,
                        &page_label_for_load,
                    );
                    let selected = selected_for_load.get();
                    refresh_selected_button_state(&key_buttons_for_load, selected);
                    let icons = icons_for_load.borrow();
                    let backgrounds = backgrounds_for_load.borrow();
                    refresh_key_grid(
                        &state_for_load,
                        &key_buttons_for_load,
                        &key_pictures_for_load,
                        current_page_for_load.get(),
                        backgrounds.as_slice(),
                    );
                    populate_editor_guarded(
                        &state_for_load,
                        current_page_for_load.get(),
                        selected,
                        &widgets_for_load,
                        icons.as_slice(),
                        backgrounds.as_slice(),
                        &editor_syncing_for_load,
                    );
                    let profile = state_for_load.borrow().profile.clone();
                    widgets_for_load
                        .status_label
                        .set_text(&format!("Loaded '{}' (profile: {profile})", path.display()));
                }
                Err(err) => widgets_for_load.status_label.set_text(&err),
            }
        });
    }

    {
        let state_for_save = state.clone();
        let current_page_for_save = current_page.clone();
        let selected_for_save = selected_key.clone();
        let widgets_for_save = widgets.clone();
        let icons_for_save = icon_names.clone();
        let backgrounds_for_save = clock_backgrounds.clone();
        let prev_for_save = prev_page_button.clone();
        let next_for_save = next_page_button.clone();
        let page_label_for_save = page_label.clone();
        let key_buttons_for_save = key_buttons.clone();
        let key_pictures_for_save = key_pictures.clone();
        let editor_syncing_for_save = editor_syncing.clone();

        save_button.connect_clicked(move |_| {
            let selected_slot = selected_for_save.get();
            let current_page_index = current_page_for_save.get();
            let icons = icons_for_save.borrow();
            let backgrounds = backgrounds_for_save.borrow();
            apply_editor_to_selected_key(
                &state_for_save,
                current_page_index,
                selected_slot,
                &widgets_for_save,
                icons.as_slice(),
                backgrounds.as_slice(),
            );
            refresh_key_grid(
                &state_for_save,
                &key_buttons_for_save,
                &key_pictures_for_save,
                current_page_index,
                backgrounds.as_slice(),
            );

            let path = PathBuf::from(widgets_for_save.config_path_entry.text().as_str());
            let (config, profile_changed) = {
                let mut state = state_for_save.borrow_mut();
                let previous_profile = state.profile.clone();
                update_state_profile_paths(&mut state, &path);
                (state.config.clone(), state.profile != previous_profile)
            };
            drop(icons);
            drop(backgrounds);

            if profile_changed {
                refresh_icon_catalogs(
                    &state_for_save,
                    &icons_for_save,
                    &backgrounds_for_save,
                    &widgets_for_save,
                );
                clamp_page_and_selection(
                    &state_for_save,
                    &current_page_for_save,
                    &selected_for_save,
                );
                refresh_page_controls(
                    &state_for_save,
                    &current_page_for_save,
                    &prev_for_save,
                    &next_for_save,
                    &page_label_for_save,
                );
                let selected_slot = selected_for_save.get();
                refresh_selected_button_state(&key_buttons_for_save, selected_slot);
                let icons = icons_for_save.borrow();
                let backgrounds = backgrounds_for_save.borrow();
                refresh_key_grid(
                    &state_for_save,
                    &key_buttons_for_save,
                    &key_pictures_for_save,
                    current_page_for_save.get(),
                    backgrounds.as_slice(),
                );
                populate_editor_guarded(
                    &state_for_save,
                    current_page_for_save.get(),
                    selected_slot,
                    &widgets_for_save,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                    &editor_syncing_for_save,
                );
            }

            match save_config(&path, &config) {
                Ok(()) => widgets_for_save
                    .status_label
                    .set_text(&format!("Saved '{}'", path.display())),
                Err(err) => widgets_for_save.status_label.set_text(&err),
            }
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
            widgets_for_add_key
                .status_label
                .set_text(&format!("Added button {}", new_key_index + 1));
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
                .title("Add icon")
                .transient_for(&window_for_add_icon)
                .modal(true)
                .action(gtk::FileChooserAction::Open)
                .accept_label("Add")
                .cancel_label("Cancel")
                .build();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Images"));
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
                                editor_syncing_for_response.set(true);
                                widgets_for_response.icon_kind_dropdown.set_selected(0);
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
                                widgets_for_response.status_label.set_text(&format!(
                                    "Added and selected icon '{}' in '{}'",
                                    icon_name,
                                    writable_image_dir.display()
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
