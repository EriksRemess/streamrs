fn finalize_and_present(
    window: &ApplicationWindow,
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
    editor_syncing: &Rc<Cell<bool>>,
) {
    clamp_page_and_selection(&state, &current_page, &selected_key);
    refresh_page_controls(
        &state,
        &current_page,
        &prev_page_button,
        &next_page_button,
        &page_label,
    );
    let initial_backgrounds = clock_backgrounds.borrow();
    refresh_key_grid(
        &state,
        &key_buttons,
        &key_pictures,
        current_page.get(),
        initial_backgrounds.as_slice(),
    );
    refresh_selected_button_state(&key_buttons, selected_key.get());
    let initial_icons = icon_names.borrow();
    populate_editor_guarded(
        &state,
        current_page.get(),
        selected_key.get(),
        &widgets,
        initial_icons.as_slice(),
        initial_backgrounds.as_slice(),
        &editor_syncing,
    );

    window.present();
}
