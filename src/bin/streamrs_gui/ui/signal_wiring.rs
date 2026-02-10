include!("signals/editor.rs");
include!("signals/navigation.rs");
include!("signals/primary_actions.rs");
include!("signals/management.rs");
include!("signals/clock.rs");
include!("signals/finalize.rs");

fn wire_ui_handlers_and_present(
    window: &ApplicationWindow,
    state: Rc<RefCell<AppState>>,
    current_page: Rc<Cell<usize>>,
    selected_key: Rc<Cell<usize>>,
    widgets: EditorWidgets,
    icon_names: Rc<RefCell<Vec<String>>>,
    clock_backgrounds: Rc<RefCell<Vec<String>>>,
    key_buttons: Vec<Button>,
    key_pictures: Vec<Picture>,
    prev_page_button: Button,
    next_page_button: Button,
    page_label: Label,
    load_button: Button,
    save_button: Button,
    add_key_button: Button,
    add_icon_button: Button,
    apply_button: Button,
    clear_button: Button,
) {
    let editor_syncing = Rc::new(Cell::new(false));

    wire_editor_dropdown_signals(
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &editor_syncing,
    );

    wire_navigation_signals(
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &prev_page_button,
        &next_page_button,
        &page_label,
        &editor_syncing,
    );

    wire_primary_action_signals(
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &prev_page_button,
        &next_page_button,
        &page_label,
        &apply_button,
        &clear_button,
        &editor_syncing,
    );

    wire_management_signals(
        window,
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &prev_page_button,
        &next_page_button,
        &page_label,
        &load_button,
        &save_button,
        &add_key_button,
        &add_icon_button,
        &editor_syncing,
    );

    wire_clock_refresh_signal(
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &editor_syncing,
    );

    finalize_and_present(
        window,
        &state,
        &current_page,
        &selected_key,
        &widgets,
        &icon_names,
        &clock_backgrounds,
        &key_buttons,
        &key_pictures,
        &prev_page_button,
        &next_page_button,
        &page_label,
        &editor_syncing,
    );
}
