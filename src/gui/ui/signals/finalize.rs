use super::super::*;
use adw::prelude::*;

pub(crate) fn finalize_and_present(window: &ApplicationWindow, ctx: &UiCtx) {
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
    clamp_page_and_selection(state, current_page, selected_key);
    refresh_page_controls(
        state,
        current_page,
        prev_page_button,
        next_page_button,
        page_label,
    );
    let initial_backgrounds = clock_backgrounds.borrow();
    refresh_key_grid(
        state,
        key_buttons,
        key_pictures,
        current_page.get(),
        initial_backgrounds.as_slice(),
    );
    refresh_selected_button_state(key_buttons, selected_key.get());
    let initial_icons = icon_names.borrow();
    populate_editor_guarded(
        state,
        current_page.get(),
        selected_key.get(),
        widgets,
        initial_icons.as_slice(),
        initial_backgrounds.as_slice(),
        editor_syncing,
    );

    window.present();
}
