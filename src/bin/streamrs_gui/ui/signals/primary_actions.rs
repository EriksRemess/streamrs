fn wire_primary_action_signals(
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    selected_key: &Rc<Cell<usize>>,
    widgets: &EditorWidgets,
    icon_names: &Rc<RefCell<Vec<String>>>,
    clock_backgrounds: &Rc<RefCell<Vec<String>>>,
    key_buttons: &Vec<Button>,
    key_pictures: &Vec<Picture>,
    prev_page_button: &Button,
    next_page_button: &Button,
    page_label: &Label,
    apply_button: &Button,
    clear_button: &Button,
    editor_syncing: &Rc<Cell<bool>>,
) {
    {
        let state_for_apply = state.clone();
        let current_page_for_apply = current_page.clone();
        let selected_for_apply = selected_key.clone();
        let widgets_for_apply = widgets.clone();
        let icons_for_apply = icon_names.clone();
        let backgrounds_for_apply = clock_backgrounds.clone();
        let key_buttons_for_apply = key_buttons.clone();
        let key_pictures_for_apply = key_pictures.clone();
        let editor_syncing_for_apply = editor_syncing.clone();

        apply_button.connect_clicked(move |_| {
            let slot = selected_for_apply.get();
            let page = current_page_for_apply.get();
            let icons = icons_for_apply.borrow();
            let backgrounds = backgrounds_for_apply.borrow();
            let applied = apply_editor_to_selected_key(
                &state_for_apply,
                page,
                slot,
                &widgets_for_apply,
                icons.as_slice(),
                backgrounds.as_slice(),
            );
            refresh_key_grid(
                &state_for_apply,
                &key_buttons_for_apply,
                &key_pictures_for_apply,
                page,
                backgrounds.as_slice(),
            );
            populate_editor_guarded(
                &state_for_apply,
                page,
                slot,
                &widgets_for_apply,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_apply,
            );
            let message = if applied {
                let key_index = {
                    let mut state = state_for_apply.borrow_mut();
                    normalize_config(&mut state.config);
                    key_index_for_slot(&state.config, page, slot).map(|index| index + 1)
                };
                key_index
                    .map(|index| format!("Applied changes to key {index}"))
                    .unwrap_or_else(|| "Applied changes".to_string())
            } else {
                "This slot is reserved for page navigation".to_string()
            };
            widgets_for_apply.status_label.set_text(&message);
        });
    }

    {
        let state_for_clear = state.clone();
        let current_page_for_clear = current_page.clone();
        let selected_for_clear = selected_key.clone();
        let widgets_for_clear = widgets.clone();
        let icons_for_clear = icon_names.clone();
        let backgrounds_for_clear = clock_backgrounds.clone();
        let key_buttons_for_clear = key_buttons.clone();
        let key_pictures_for_clear = key_pictures.clone();
        let prev_for_clear = prev_page_button.clone();
        let next_for_clear = next_page_button.clone();
        let page_label_for_clear = page_label.clone();
        let editor_syncing_for_clear = editor_syncing.clone();

        clear_button.connect_clicked(move |_| {
            let slot = selected_for_clear.get();
            let cleared =
                clear_selected_key(&state_for_clear, current_page_for_clear.get(), slot);
            if cleared {
                clamp_page_and_selection(
                    &state_for_clear,
                    &current_page_for_clear,
                    &selected_for_clear,
                );
                refresh_page_controls(
                    &state_for_clear,
                    &current_page_for_clear,
                    &prev_for_clear,
                    &next_for_clear,
                    &page_label_for_clear,
                );
            }
            let page = current_page_for_clear.get();
            let selected = selected_for_clear.get();
            let icons = icons_for_clear.borrow();
            let backgrounds = backgrounds_for_clear.borrow();
            refresh_selected_button_state(&key_buttons_for_clear, selected);
            refresh_key_grid(
                &state_for_clear,
                &key_buttons_for_clear,
                &key_pictures_for_clear,
                page,
                backgrounds.as_slice(),
            );
            populate_editor_guarded(
                &state_for_clear,
                page,
                selected,
                &widgets_for_clear,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_clear,
            );
            if cleared {
                widgets_for_clear.status_label.set_text("Deleted selected key");
            } else {
                widgets_for_clear
                    .status_label
                    .set_text("Navigation buttons cannot be deleted");
            }
        });
    }
}
