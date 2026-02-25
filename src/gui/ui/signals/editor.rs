use super::super::*;

pub(crate) fn wire_editor_dropdown_signals(ctx: &UiCtx) {
    let state = &ctx.state;
    let current_page = &ctx.current_page;
    let selected_key = &ctx.selected_key;
    let widgets = &ctx.widgets;
    let icon_names = &ctx.icon_names;
    let clock_backgrounds = &ctx.clock_backgrounds;
    let key_buttons = &ctx.key_buttons;
    let key_pictures = &ctx.key_pictures;
    let editor_syncing = &ctx.editor_syncing;
    {
        let state_for_kind = state.clone();
        let current_page_for_kind = current_page.clone();
        let selected_for_kind = selected_key.clone();
        let widgets_for_kind = widgets.clone();
        let icons_for_kind = icon_names.clone();
        let backgrounds_for_kind = clock_backgrounds.clone();
        let key_buttons_for_kind = key_buttons.clone();
        let key_pictures_for_kind = key_pictures.clone();
        let editor_syncing_for_kind = editor_syncing.clone();
        widgets
            .icon_kind_dropdown
            .connect_selected_notify(move |_| {
                if editor_syncing_for_kind.get() {
                    return;
                }
                set_editor_mode_visibility(&widgets_for_kind, editor_mode(&widgets_for_kind));

                let page = current_page_for_kind.get();
                let slot = selected_for_kind.get();
                let icons = icons_for_kind.borrow();
                let backgrounds = backgrounds_for_kind.borrow();
                apply_editor_to_selected_key(
                    &state_for_kind,
                    page,
                    slot,
                    &widgets_for_kind,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                );
                refresh_key_grid(
                    &state_for_kind,
                    &key_buttons_for_kind,
                    &key_pictures_for_kind,
                    page,
                    backgrounds.as_slice(),
                );
                editor_refresh_preview(
                    &state_for_kind,
                    &widgets_for_kind,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                );
                populate_editor_guarded(
                    &state_for_kind,
                    page,
                    slot,
                    &widgets_for_kind,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                    &editor_syncing_for_kind,
                );
            });
    }

    {
        let state_for_icon = state.clone();
        let current_page_for_icon = current_page.clone();
        let selected_for_icon = selected_key.clone();
        let widgets_for_icon = widgets.clone();
        let icons_for_icon = icon_names.clone();
        let backgrounds_for_icon = clock_backgrounds.clone();
        let key_buttons_for_icon = key_buttons.clone();
        let key_pictures_for_icon = key_pictures.clone();
        let editor_syncing_for_icon = editor_syncing.clone();
        widgets.icon_dropdown.connect_selected_notify(move |_| {
            if editor_syncing_for_icon.get() {
                return;
            }
            let page = current_page_for_icon.get();
            let slot = selected_for_icon.get();
            let icons = icons_for_icon.borrow();
            let backgrounds = backgrounds_for_icon.borrow();
            let applied = apply_editor_to_selected_key(
                &state_for_icon,
                page,
                slot,
                &widgets_for_icon,
                icons.as_slice(),
                backgrounds.as_slice(),
            );
            refresh_key_grid(
                &state_for_icon,
                &key_buttons_for_icon,
                &key_pictures_for_icon,
                page,
                backgrounds.as_slice(),
            );
            editor_refresh_preview(
                &state_for_icon,
                &widgets_for_icon,
                icons.as_slice(),
                backgrounds.as_slice(),
            );
            if applied {
                widgets_for_icon.status_label.set_text("Icon updated");
            }
        });
    }

    {
        let state_for_background = state.clone();
        let current_page_for_background = current_page.clone();
        let selected_for_background = selected_key.clone();
        let widgets_for_background = widgets.clone();
        let icons_for_background = icon_names.clone();
        let backgrounds_for_background = clock_backgrounds.clone();
        let key_buttons_for_background = key_buttons.clone();
        let key_pictures_for_background = key_pictures.clone();
        let editor_syncing_for_background = editor_syncing.clone();
        widgets
            .clock_background_dropdown
            .connect_selected_notify(move |_| {
                if editor_syncing_for_background.get() {
                    return;
                }
                let page = current_page_for_background.get();
                let slot = selected_for_background.get();
                let icons = icons_for_background.borrow();
                let backgrounds = backgrounds_for_background.borrow();
                let applied = apply_editor_to_selected_key(
                    &state_for_background,
                    page,
                    slot,
                    &widgets_for_background,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                );
                refresh_key_grid(
                    &state_for_background,
                    &key_buttons_for_background,
                    &key_pictures_for_background,
                    page,
                    backgrounds.as_slice(),
                );
                editor_refresh_preview(
                    &state_for_background,
                    &widgets_for_background,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                );
                if applied {
                    widgets_for_background
                        .status_label
                        .set_text("Clock background updated");
                }
            });
    }

    {
        let state_for_icon_on = state.clone();
        let current_page_for_icon_on = current_page.clone();
        let selected_for_icon_on = selected_key.clone();
        let widgets_for_icon_on = widgets.clone();
        let icons_for_icon_on = icon_names.clone();
        let backgrounds_for_icon_on = clock_backgrounds.clone();
        let key_buttons_for_icon_on = key_buttons.clone();
        let key_pictures_for_icon_on = key_pictures.clone();
        let editor_syncing_for_icon_on = editor_syncing.clone();
        widgets.icon_on_dropdown.connect_selected_notify(move |_| {
            if editor_syncing_for_icon_on.get() {
                return;
            }
            let page = current_page_for_icon_on.get();
            let slot = selected_for_icon_on.get();
            let icons = icons_for_icon_on.borrow();
            let backgrounds = backgrounds_for_icon_on.borrow();
            if apply_editor_to_selected_key(
                &state_for_icon_on,
                page,
                slot,
                &widgets_for_icon_on,
                icons.as_slice(),
                backgrounds.as_slice(),
            ) {
                refresh_key_grid(
                    &state_for_icon_on,
                    &key_buttons_for_icon_on,
                    &key_pictures_for_icon_on,
                    page,
                    backgrounds.as_slice(),
                );
                widgets_for_icon_on
                    .status_label
                    .set_text("Status-on icon updated");
            }
        });
    }

    {
        let state_for_icon_off = state.clone();
        let current_page_for_icon_off = current_page.clone();
        let selected_for_icon_off = selected_key.clone();
        let widgets_for_icon_off = widgets.clone();
        let icons_for_icon_off = icon_names.clone();
        let backgrounds_for_icon_off = clock_backgrounds.clone();
        let key_buttons_for_icon_off = key_buttons.clone();
        let key_pictures_for_icon_off = key_pictures.clone();
        let editor_syncing_for_icon_off = editor_syncing.clone();
        widgets.icon_off_dropdown.connect_selected_notify(move |_| {
            if editor_syncing_for_icon_off.get() {
                return;
            }
            let page = current_page_for_icon_off.get();
            let slot = selected_for_icon_off.get();
            let icons = icons_for_icon_off.borrow();
            let backgrounds = backgrounds_for_icon_off.borrow();
            if apply_editor_to_selected_key(
                &state_for_icon_off,
                page,
                slot,
                &widgets_for_icon_off,
                icons.as_slice(),
                backgrounds.as_slice(),
            ) {
                refresh_key_grid(
                    &state_for_icon_off,
                    &key_buttons_for_icon_off,
                    &key_pictures_for_icon_off,
                    page,
                    backgrounds.as_slice(),
                );
                widgets_for_icon_off
                    .status_label
                    .set_text("Status-off icon updated");
            }
        });
    }
}
