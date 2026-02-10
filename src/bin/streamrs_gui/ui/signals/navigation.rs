const DROP_PREVIEW_SWAP_CLASS: &str = "key-drop-swap";
const DROP_PREVIEW_BEFORE_CLASS: &str = "key-drop-before";
const DROP_PREVIEW_AFTER_CLASS: &str = "key-drop-after";

fn clear_drop_preview_state(buttons: &[Button]) {
    for button in buttons {
        button.remove_css_class(DROP_PREVIEW_SWAP_CLASS);
        button.remove_css_class(DROP_PREVIEW_BEFORE_CLASS);
        button.remove_css_class(DROP_PREVIEW_AFTER_CLASS);
    }
}

fn apply_drop_preview_state(buttons: &[Button], target_slot: usize, cursor_x: f64) {
    clear_drop_preview_state(buttons);
    let Some(target_button) = buttons.get(target_slot) else {
        return;
    };

    let width = f64::from(target_button.allocated_width().max(1));
    let insert_before_threshold = width * 0.33;
    let insert_after_threshold = width * 0.66;

    if cursor_x < insert_before_threshold {
        target_button.add_css_class(DROP_PREVIEW_BEFORE_CLASS);
    } else if cursor_x > insert_after_threshold {
        target_button.add_css_class(DROP_PREVIEW_AFTER_CLASS);
    } else {
        target_button.add_css_class(DROP_PREVIEW_SWAP_CLASS);
    }
}

fn wire_navigation_signals(
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
    for (index, button) in key_buttons.iter().enumerate() {
        let buttons = key_buttons.clone();
        let key_pictures_for_click = key_pictures.clone();
        let state_for_click = state.clone();
        let current_page_for_click = current_page.clone();
        let selected_for_click = selected_key.clone();
        let widgets_for_click = widgets.clone();
        let icons_for_click = icon_names.clone();
        let backgrounds_for_click = clock_backgrounds.clone();
        let prev_for_click = prev_page_button.clone();
        let next_for_click = next_page_button.clone();
        let page_label_for_click = page_label.clone();
        let editor_syncing_for_click = editor_syncing.clone();

        button.connect_clicked(move |_| {
            let (page, total_pages) = {
                let mut state = state_for_click.borrow_mut();
                normalize_config(&mut state.config);
                let total_pages = page_count(state.config.keys.len()).max(1);
                let page = current_page_for_click.get().min(total_pages.saturating_sub(1));
                (page, total_pages)
            };

            if let Some(nav_slot) = navigation_slot_for_slot(page, total_pages, index) {
                let target_page = match nav_slot {
                    ReservedNavigationSlot::PreviousPage => page.saturating_sub(1),
                    ReservedNavigationSlot::NextPage => {
                        (page + 1).min(total_pages.saturating_sub(1))
                    }
                };
                if target_page != page {
                    current_page_for_click.set(target_page);
                    clamp_page_and_selection(
                        &state_for_click,
                        &current_page_for_click,
                        &selected_for_click,
                    );
                    refresh_page_controls(
                        &state_for_click,
                        &current_page_for_click,
                        &prev_for_click,
                        &next_for_click,
                        &page_label_for_click,
                    );
                    let selected = selected_for_click.get();
                    refresh_selected_button_state(&buttons, selected);
                    let icons = icons_for_click.borrow();
                    let backgrounds = backgrounds_for_click.borrow();
                    refresh_key_grid(
                        &state_for_click,
                        &buttons,
                        &key_pictures_for_click,
                        current_page_for_click.get(),
                        backgrounds.as_slice(),
                    );
                    populate_editor_guarded(
                        &state_for_click,
                        current_page_for_click.get(),
                        selected,
                        &widgets_for_click,
                        icons.as_slice(),
                        backgrounds.as_slice(),
                        &editor_syncing_for_click,
                    );
                    widgets_for_click.status_label.set_text(&format!(
                        "Page {}/{}",
                        current_page_for_click.get() + 1,
                        total_pages
                    ));
                }
                return;
            }

            selected_for_click.set(index);
            refresh_selected_button_state(&buttons, index);
            let icons = icons_for_click.borrow();
            let backgrounds = backgrounds_for_click.borrow();
            populate_editor_guarded(
                &state_for_click,
                current_page_for_click.get(),
                index,
                &widgets_for_click,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_click,
            );
        });

        {
            let state_for_drag_source = state.clone();
            let current_page_for_drag_source = current_page.clone();
            let key_picture_for_drag = key_pictures[index].clone();
            let drag_source = gtk::DragSource::new();
            drag_source.set_actions(gtk::gdk::DragAction::MOVE);
            drag_source.connect_drag_begin(move |source, _| {
                let width = key_picture_for_drag.allocated_width().max(1);
                let height = key_picture_for_drag.allocated_height().max(1);
                let widget_paintable = gtk::WidgetPaintable::new(Some(&key_picture_for_drag));
                source.set_icon(Some(&widget_paintable), width / 2, height / 2);
            });
            drag_source.connect_prepare(move |_, _, _| {
                let mut state = state_for_drag_source.borrow_mut();
                normalize_config(&mut state.config);
                let total_pages = page_count(state.config.keys.len()).max(1);
                let page = current_page_for_drag_source
                    .get()
                    .min(total_pages.saturating_sub(1));
                if key_index_for_slot(&state.config, page, index).is_none() {
                    return None;
                }
                Some(gtk::gdk::ContentProvider::for_value(&(index as u32).to_value()))
            });
            button.add_controller(drag_source);
        }

        {
            let buttons_for_drop = key_buttons.clone();
            let key_pictures_for_drop = key_pictures.clone();
            let state_for_drop = state.clone();
            let current_page_for_drop = current_page.clone();
            let selected_for_drop = selected_key.clone();
            let widgets_for_drop = widgets.clone();
            let icons_for_drop = icon_names.clone();
            let backgrounds_for_drop = clock_backgrounds.clone();
            let editor_syncing_for_drop = editor_syncing.clone();
            let drop_button = button.clone();
            let buttons_for_motion = key_buttons.clone();
            let buttons_for_leave = key_buttons.clone();
            let drop_target = gtk::DropTarget::new(gtk::glib::Type::U32, gtk::gdk::DragAction::MOVE);
            drop_target.connect_motion(move |_, x, _| {
                apply_drop_preview_state(&buttons_for_motion, index, x);
                gtk::gdk::DragAction::MOVE
            });
            drop_target.connect_leave(move |_| {
                clear_drop_preview_state(&buttons_for_leave);
            });
            drop_target.connect_drop(move |_, value, x, _| {
                clear_drop_preview_state(&buttons_for_drop);
                let Ok(source_slot) = value.get::<u32>() else {
                    return false;
                };
                let source_slot = source_slot as usize;
                if source_slot == index {
                    return false;
                }
                let page = current_page_for_drop.get();
                let width = f64::from(drop_button.allocated_width().max(1));
                let insert_before_threshold = width * 0.33;
                let insert_after_threshold = width * 0.66;

                let operation_message = if x < insert_before_threshold {
                    if move_key_between_slots(&state_for_drop, page, source_slot, index, false) {
                        "Inserted button before target"
                    } else {
                        return false;
                    }
                } else if x > insert_after_threshold {
                    if move_key_between_slots(&state_for_drop, page, source_slot, index, true) {
                        "Inserted button after target"
                    } else {
                        return false;
                    }
                } else if swap_keys_between_slots(&state_for_drop, page, source_slot, index) {
                    "Swapped buttons"
                } else {
                    return false;
                };

                selected_for_drop.set(index);
                refresh_selected_button_state(&buttons_for_drop, index);
                let icons = icons_for_drop.borrow();
                let backgrounds = backgrounds_for_drop.borrow();
                refresh_key_grid(
                    &state_for_drop,
                    &buttons_for_drop,
                    &key_pictures_for_drop,
                    page,
                    backgrounds.as_slice(),
                );
                populate_editor_guarded(
                    &state_for_drop,
                    page,
                    index,
                    &widgets_for_drop,
                    icons.as_slice(),
                    backgrounds.as_slice(),
                    &editor_syncing_for_drop,
                );
                widgets_for_drop.status_label.set_text(operation_message);
                true
            });
            button.add_controller(drop_target);
        }
    }

    {
        let state_for_prev_page = state.clone();
        let current_page_for_prev_page = current_page.clone();
        let selected_for_prev_page = selected_key.clone();
        let widgets_for_prev_page = widgets.clone();
        let icons_for_prev_page = icon_names.clone();
        let backgrounds_for_prev_page = clock_backgrounds.clone();
        let key_buttons_for_prev_page = key_buttons.clone();
        let key_pictures_for_prev_page = key_pictures.clone();
        let prev_for_prev_page = prev_page_button.clone();
        let next_for_prev_page = next_page_button.clone();
        let page_label_for_prev_page = page_label.clone();
        let editor_syncing_for_prev_page = editor_syncing.clone();

        prev_page_button.connect_clicked(move |_| {
            if current_page_for_prev_page.get() == 0 {
                return;
            }
            current_page_for_prev_page.set(current_page_for_prev_page.get().saturating_sub(1));
            clamp_page_and_selection(
                &state_for_prev_page,
                &current_page_for_prev_page,
                &selected_for_prev_page,
            );
            refresh_page_controls(
                &state_for_prev_page,
                &current_page_for_prev_page,
                &prev_for_prev_page,
                &next_for_prev_page,
                &page_label_for_prev_page,
            );

            let selected = selected_for_prev_page.get();
            refresh_selected_button_state(&key_buttons_for_prev_page, selected);
            let icons = icons_for_prev_page.borrow();
            let backgrounds = backgrounds_for_prev_page.borrow();
            refresh_key_grid(
                &state_for_prev_page,
                &key_buttons_for_prev_page,
                &key_pictures_for_prev_page,
                current_page_for_prev_page.get(),
                backgrounds.as_slice(),
            );
            populate_editor_guarded(
                &state_for_prev_page,
                current_page_for_prev_page.get(),
                selected,
                &widgets_for_prev_page,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_prev_page,
            );
        });
    }

    {
        let state_for_next_page = state.clone();
        let current_page_for_next_page = current_page.clone();
        let selected_for_next_page = selected_key.clone();
        let widgets_for_next_page = widgets.clone();
        let icons_for_next_page = icon_names.clone();
        let backgrounds_for_next_page = clock_backgrounds.clone();
        let key_buttons_for_next_page = key_buttons.clone();
        let key_pictures_for_next_page = key_pictures.clone();
        let prev_for_next_page = prev_page_button.clone();
        let next_for_next_page = next_page_button.clone();
        let page_label_for_next_page = page_label.clone();
        let editor_syncing_for_next_page = editor_syncing.clone();

        next_page_button.connect_clicked(move |_| {
            current_page_for_next_page.set(current_page_for_next_page.get().saturating_add(1));
            clamp_page_and_selection(
                &state_for_next_page,
                &current_page_for_next_page,
                &selected_for_next_page,
            );
            refresh_page_controls(
                &state_for_next_page,
                &current_page_for_next_page,
                &prev_for_next_page,
                &next_for_next_page,
                &page_label_for_next_page,
            );

            let selected = selected_for_next_page.get();
            refresh_selected_button_state(&key_buttons_for_next_page, selected);
            let icons = icons_for_next_page.borrow();
            let backgrounds = backgrounds_for_next_page.borrow();
            refresh_key_grid(
                &state_for_next_page,
                &key_buttons_for_next_page,
                &key_pictures_for_next_page,
                current_page_for_next_page.get(),
                backgrounds.as_slice(),
            );
            populate_editor_guarded(
                &state_for_next_page,
                current_page_for_next_page.get(),
                selected,
                &widgets_for_next_page,
                icons.as_slice(),
                backgrounds.as_slice(),
                &editor_syncing_for_next_page,
            );
        });
    }
}
