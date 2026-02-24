use crate::gui::*;

#[derive(Clone)]
pub(super) struct UiCtx {
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
    editor_syncing: Rc<Cell<bool>>,
}

#[path = "signals/mod.rs"]
mod signals;
use self::signals::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn wire_ui_handlers_and_present(
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
    let ctx = UiCtx {
        state,
        current_page,
        selected_key,
        widgets,
        icon_names,
        clock_backgrounds,
        key_buttons,
        key_pictures,
        prev_page_button,
        next_page_button,
        page_label,
        editor_syncing: Rc::new(Cell::new(false)),
    };

    wire_editor_dropdown_signals(&ctx);
    wire_navigation_signals(&ctx);
    wire_primary_action_signals(&ctx, &apply_button, &clear_button);
    wire_management_signals(
        window,
        &ctx,
        &load_button,
        &save_button,
        &add_key_button,
        &add_icon_button,
    );
    wire_clock_refresh_signal(&ctx);
    finalize_and_present(window, &ctx);
}
