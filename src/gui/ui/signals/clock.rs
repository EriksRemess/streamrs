use super::super::*;

pub(crate) fn wire_clock_refresh_signal(ctx: &UiCtx) {
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
        let state_for_clock = state.clone();
        let current_page_for_clock = current_page.clone();
        let selected_for_clock = selected_key.clone();
        let widgets_for_clock = widgets.clone();
        let icons_for_clock = icon_names.clone();
        let backgrounds_for_clock = clock_backgrounds.clone();
        let key_buttons_for_clock = key_buttons.clone();
        let key_pictures_for_clock = key_pictures.clone();
        let last_clock_text = Rc::new(RefCell::new(String::new()));
        let last_clock_for_tick = last_clock_text.clone();
        let editor_syncing_for_clock = editor_syncing.clone();

        gtk::glib::timeout_add_seconds_local(1, move || {
            let has_clock = {
                let mut state = state_for_clock.borrow_mut();
                normalize_config(&mut state.config);
                config_uses_clock(&state.config)
            };

            if has_clock {
                let now_clock = current_clock_text();
                if *last_clock_for_tick.borrow() == now_clock {
                    return gtk::glib::ControlFlow::Continue;
                }
                *last_clock_for_tick.borrow_mut() = now_clock;

                let icons = icons_for_clock.borrow();
                let backgrounds = backgrounds_for_clock.borrow();
                refresh_key_grid(
                    &state_for_clock,
                    &key_buttons_for_clock,
                    &key_pictures_for_clock,
                    current_page_for_clock.get(),
                    backgrounds.as_slice(),
                );
                populate_editor_guarded(
                    &state_for_clock,
                    current_page_for_clock.get(),
                    selected_for_clock.get(),
                    &widgets_for_clock,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                    &editor_syncing_for_clock,
                );
            }

            gtk::glib::ControlFlow::Continue
        });
    }
}
