use super::*;

pub(crate) fn is_plain_blank_key(key: &KeyBinding) -> bool {
    key.status.is_none()
        && key.icon_on.is_none()
        && key.icon_off.is_none()
        && is_blank_background_icon_name(&key.icon)
}

pub(crate) fn slot_has_plain_blank_key(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    selected_slot: usize,
) -> bool {
    let mut state = state.borrow_mut();
    normalize_config(&mut state.config);
    key_index_for_slot(&state.config, current_page, selected_slot)
        .and_then(|index| state.config.keys.get(index))
        .is_some_and(is_plain_blank_key)
}

pub(crate) fn refresh_key_grid(
    state: &Rc<RefCell<AppState>>,
    key_buttons: &[Button],
    key_pictures: &[Picture],
    current_page: usize,
    clock_backgrounds: &[String],
) {
    let (config, image_dirs, page, total_pages) = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        let total_pages = page_count(&state.config).max(1);
        let page = current_page.min(total_pages.saturating_sub(1));
        (
            state.config.clone(),
            state.image_dirs.clone(),
            page,
            total_pages,
        )
    };

    for (slot, picture) in key_pictures.iter().enumerate() {
        if let Some(key_index) = key_index_for_slot(&config, page, slot) {
            if let Some(key) = config.keys.get(key_index) {
                set_picture_icon(picture, &image_dirs, key, clock_backgrounds);
                if is_plain_blank_key(key) {
                    key_buttons[slot].add_css_class("key-blank-binding");
                } else {
                    key_buttons[slot].remove_css_class("key-blank-binding");
                }
                let tip = if is_plain_blank_key(key) {
                    trf(
                        "Button {button} (page {page}, slot {slot}) - Blank button",
                        &[
                            ("button", (key_index + 1).to_string()),
                            ("page", (page + 1).to_string()),
                            ("slot", (slot + 1).to_string()),
                        ],
                    )
                } else {
                    trf(
                        "Button {button} (page {page}, slot {slot})",
                        &[
                            ("button", (key_index + 1).to_string()),
                            ("page", (page + 1).to_string()),
                            ("slot", (slot + 1).to_string()),
                        ],
                    )
                };
                key_buttons[slot].set_tooltip_text(Some(&tip));
            }
            key_buttons[slot].set_sensitive(true);
            key_buttons[slot].add_css_class("key-has-binding");
            key_buttons[slot].remove_css_class("key-navigation-slot");
            key_buttons[slot].remove_css_class("key-empty-slot");
            continue;
        }

        if let Some(nav_slot) = navigation_slot_for_slot(&config, page, total_pages, slot) {
            key_buttons[slot].set_sensitive(true);
            key_buttons[slot].remove_css_class("key-has-binding");
            key_buttons[slot].remove_css_class("key-blank-binding");
            key_buttons[slot].add_css_class("key-navigation-slot");
            key_buttons[slot].remove_css_class("key-empty-slot");
            let icon_name = navigation_icon_name(nav_slot);
            let icon_path = find_icon_file(&image_dirs, icon_name);
            update_picture_file(picture, icon_path.as_deref());
            let tip = match nav_slot {
                ReservedNavigationSlot::PreviousPage => tr("Previous page"),
                ReservedNavigationSlot::NextPage => tr("Next page"),
            };
            key_buttons[slot].set_tooltip_text(Some(&tip));
            picture.set_tooltip_text(Some(&tip));
            continue;
        }

        key_buttons[slot].set_sensitive(false);
        key_buttons[slot].remove_css_class("key-has-binding");
        key_buttons[slot].remove_css_class("key-blank-binding");
        key_buttons[slot].remove_css_class("key-navigation-slot");
        key_buttons[slot].add_css_class("key-empty-slot");
        key_buttons[slot].set_tooltip_text(Some(&tr("Reserved for page navigation in streamrs")));
        picture.set_tooltip_text(None);
        update_picture_file(picture, None);
    }
}

pub(crate) fn editor_mode(widgets: &EditorWidgets) -> EditorMode {
    editor_mode_from_index(widgets.icon_kind_dropdown.selected())
}

pub(crate) fn editor_mode_from_index(selected: u32) -> EditorMode {
    match selected {
        1 => EditorMode::Regular,
        2 => EditorMode::Status,
        3 => EditorMode::Clock,
        4 => EditorMode::Calendar,
        _ => EditorMode::Blank,
    }
}

pub(crate) fn set_editor_mode_visibility(widgets: &EditorWidgets, mode: EditorMode) {
    let is_regular = mode == EditorMode::Regular;
    let is_status = mode == EditorMode::Status;
    let is_clock = mode == EditorMode::Clock;

    widgets.icon_label.set_visible(is_regular);
    widgets.icon_row.set_visible(is_regular);
    widgets.status_command_label.set_visible(is_status);
    widgets.status_entry.set_visible(is_status);
    widgets.icon_on_label.set_visible(is_status);
    widgets.icon_on_dropdown.set_visible(is_status);
    widgets.icon_off_label.set_visible(is_status);
    widgets.icon_off_dropdown.set_visible(is_status);
    widgets.interval_label.set_visible(is_status);
    widgets.interval_spin.set_visible(is_status);

    widgets.clock_background_label.set_visible(is_clock);
    widgets.clock_background_dropdown.set_visible(is_clock);
}

pub(crate) fn set_editor_controls_sensitive(widgets: &EditorWidgets, enabled: bool) {
    widgets.action_entry.set_sensitive(enabled);
    widgets.icon_kind_dropdown.set_sensitive(enabled);
    widgets.icon_row.set_sensitive(enabled);
    widgets.clock_background_dropdown.set_sensitive(enabled);
    widgets.status_entry.set_sensitive(enabled);
    widgets.icon_on_dropdown.set_sensitive(enabled);
    widgets.icon_off_dropdown.set_sensitive(enabled);
    widgets.interval_spin.set_sensitive(enabled);
    widgets.apply_button.set_sensitive(enabled);
    widgets.clear_button.set_sensitive(enabled);
}

pub(crate) fn preview_key_from_editor(
    widgets: &EditorWidgets,
    icon_names: &[String],
    clock_backgrounds: &[String],
) -> KeyBinding {
    let mut key = KeyBinding::default();
    match editor_mode(widgets) {
        EditorMode::Blank => {
            key.icon = CLOCK_BACKGROUND_ICON.to_string();
        }
        EditorMode::Regular => {
            key.icon = dropdown_selected_icon(&widgets.icon_dropdown, icon_names);
        }
        EditorMode::Status => {
            key.icon = dropdown_selected_icon(&widgets.icon_off_dropdown, icon_names);
            key.icon_on = Some(dropdown_selected_icon(
                &widgets.icon_on_dropdown,
                icon_names,
            ));
            key.icon_off = Some(dropdown_selected_icon(
                &widgets.icon_off_dropdown,
                icon_names,
            ));
            key.status = trimmed_or_none(widgets.status_entry.text().as_str());
        }
        EditorMode::Clock => {
            key.icon = CLOCK_ICON_ALIAS.to_string();
            let selected =
                dropdown_selected_icon(&widgets.clock_background_dropdown, clock_backgrounds);
            if selected != CLOCK_BACKGROUND_ICON {
                key.clock_background = Some(selected);
            }
        }
        EditorMode::Calendar => {
            key.icon = CALENDAR_ICON_ALIAS.to_string();
        }
    }
    key
}

pub(crate) fn populate_editor(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    selected_slot: usize,
    widgets: &EditorWidgets,
    icon_names: &[String],
    clock_backgrounds: &[String],
) {
    let (key, image_dirs, key_index, nav_slot, total_pages, page) = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        let total_pages = page_count(&state.config).max(1);
        let page = current_page.min(total_pages.saturating_sub(1));
        let key_index = key_index_for_slot(&state.config, page, selected_slot);
        let nav_slot = navigation_slot_for_slot(&state.config, page, total_pages, selected_slot);
        let key = key_index
            .and_then(|index| state.config.keys.get(index).cloned())
            .unwrap_or_default();
        (
            key,
            state.image_dirs.clone(),
            key_index,
            nav_slot,
            total_pages,
            page,
        )
    };

    set_editor_controls_sensitive(widgets, key_index.is_some());

    if let Some(key_index) = key_index {
        widgets.selected_label.set_text(&trf(
            "Editing {ordinal} button",
            &[("ordinal", tr_ordinal(key_index + 1))],
        ));
        widgets
            .action_entry
            .set_text(key.action.as_deref().unwrap_or_default());

        let mode = if icon_is_clock(&key.icon) {
            EditorMode::Clock
        } else if icon_is_calendar(&key.icon) {
            EditorMode::Calendar
        } else if key.status.is_some() || key.icon_on.is_some() || key.icon_off.is_some() {
            EditorMode::Status
        } else if is_blank_background_icon_name(&key.icon) {
            EditorMode::Blank
        } else {
            EditorMode::Regular
        };
        widgets.icon_kind_dropdown.set_selected(match mode {
            EditorMode::Blank => 0,
            EditorMode::Regular => 1,
            EditorMode::Status => 2,
            EditorMode::Clock => 3,
            EditorMode::Calendar => 4,
        });
        set_editor_mode_visibility(widgets, mode);

        widgets
            .status_entry
            .set_text(key.status.as_deref().unwrap_or_default());
        match mode {
            EditorMode::Blank => {
                widgets.icon_dropdown.set_selected(0);
                widgets.icon_on_dropdown.set_selected(0);
                widgets.icon_off_dropdown.set_selected(0);
                set_dropdown_icon(
                    &widgets.clock_background_dropdown,
                    clock_backgrounds,
                    CLOCK_BACKGROUND_ICON,
                );
            }
            EditorMode::Regular => {
                set_dropdown_icon(&widgets.icon_dropdown, icon_names, &key.icon);
                widgets.icon_on_dropdown.set_selected(0);
                widgets.icon_off_dropdown.set_selected(0);
                set_dropdown_icon(
                    &widgets.clock_background_dropdown,
                    clock_backgrounds,
                    CLOCK_BACKGROUND_ICON,
                );
            }
            EditorMode::Status => {
                widgets.icon_dropdown.set_selected(0);
                let icon_on = key.icon_on.as_deref().unwrap_or(&key.icon);
                set_dropdown_icon(&widgets.icon_on_dropdown, icon_names, icon_on);
                let icon_off = key.icon_off.as_deref().unwrap_or(&key.icon);
                set_dropdown_icon(&widgets.icon_off_dropdown, icon_names, icon_off);
                set_dropdown_icon(
                    &widgets.clock_background_dropdown,
                    clock_backgrounds,
                    CLOCK_BACKGROUND_ICON,
                );
            }
            EditorMode::Clock => {
                widgets.icon_dropdown.set_selected(0);
                widgets.icon_on_dropdown.set_selected(0);
                widgets.icon_off_dropdown.set_selected(0);
                let background = key_clock_background_name(&key, clock_backgrounds);
                set_dropdown_icon(
                    &widgets.clock_background_dropdown,
                    clock_backgrounds,
                    background,
                );
            }
            EditorMode::Calendar => {
                widgets.icon_dropdown.set_selected(0);
                widgets.icon_on_dropdown.set_selected(0);
                widgets.icon_off_dropdown.set_selected(0);
                set_dropdown_icon(
                    &widgets.clock_background_dropdown,
                    clock_backgrounds,
                    CLOCK_BACKGROUND_ICON,
                );
            }
        }

        let interval = key
            .status_interval_ms
            .unwrap_or(DEFAULT_STATUS_INTERVAL_MS)
            .clamp(MIN_STATUS_INTERVAL_MS, MAX_STATUS_INTERVAL_MS);
        widgets.interval_spin.set_value(interval as f64);

        set_picture_icon(&widgets.icon_preview, &image_dirs, &key, clock_backgrounds);
    } else if let Some(nav_slot) = nav_slot {
        let (label, tooltip) = match nav_slot {
            ReservedNavigationSlot::PreviousPage => {
                (tr("Page navigation: Previous"), tr("Go to previous page"))
            }
            ReservedNavigationSlot::NextPage => {
                (tr("Page navigation: Next"), tr("Go to next page"))
            }
        };
        widgets.selected_label.set_text(&label);
        widgets.action_entry.set_text("");
        widgets.status_entry.set_text("");
        widgets
            .interval_spin
            .set_value(DEFAULT_STATUS_INTERVAL_MS as f64);
        widgets.icon_kind_dropdown.set_selected(0);
        set_editor_mode_visibility(widgets, EditorMode::Blank);
        widgets.icon_dropdown.set_selected(0);
        widgets.icon_on_dropdown.set_selected(0);
        widgets.icon_off_dropdown.set_selected(0);
        set_dropdown_icon(
            &widgets.clock_background_dropdown,
            clock_backgrounds,
            CLOCK_BACKGROUND_ICON,
        );
        let icon_name = navigation_icon_name(nav_slot);
        let icon_path = find_icon_file(&image_dirs, icon_name);
        update_picture_file(&widgets.icon_preview, icon_path.as_deref());
        widgets.status_label.set_text(&trf(
            "{tooltip} ({page}/{total})",
            &[
                ("tooltip", tooltip),
                ("page", (page + 1).to_string()),
                ("total", total_pages.to_string()),
            ],
        ));
    } else {
        widgets.selected_label.set_text(&tr("Reserved slot"));
        widgets.action_entry.set_text("");
        widgets.status_entry.set_text("");
        widgets
            .interval_spin
            .set_value(DEFAULT_STATUS_INTERVAL_MS as f64);
        widgets.icon_kind_dropdown.set_selected(0);
        set_editor_mode_visibility(widgets, EditorMode::Blank);
        widgets.icon_dropdown.set_selected(0);
        widgets.icon_on_dropdown.set_selected(0);
        widgets.icon_off_dropdown.set_selected(0);
        set_dropdown_icon(
            &widgets.clock_background_dropdown,
            clock_backgrounds,
            CLOCK_BACKGROUND_ICON,
        );
        update_picture_file(&widgets.icon_preview, None);
    }
}

pub(crate) fn populate_editor_guarded(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    selected_slot: usize,
    widgets: &EditorWidgets,
    icon_names: &[String],
    clock_backgrounds: &[String],
    editor_syncing: &Rc<Cell<bool>>,
) {
    editor_syncing.set(true);
    populate_editor(
        state,
        current_page,
        selected_slot,
        widgets,
        icon_names,
        clock_backgrounds,
    );
    editor_syncing.set(false);
}

pub(crate) fn trimmed_or_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn apply_editor_to_selected_key(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    selected_slot: usize,
    widgets: &EditorWidgets,
    icon_names: &[String],
    clock_backgrounds: &[String],
) -> bool {
    let action = trimmed_or_none(widgets.action_entry.text().as_str());
    let mode = editor_mode(widgets);
    let regular_icon = dropdown_selected_icon(&widgets.icon_dropdown, icon_names);
    let status = trimmed_or_none(widgets.status_entry.text().as_str());
    let icon_on_selected = dropdown_selected_icon(&widgets.icon_on_dropdown, icon_names);
    let icon_off_selected = dropdown_selected_icon(&widgets.icon_off_dropdown, icon_names);
    let interval = widgets
        .interval_spin
        .value()
        .round()
        .clamp(MIN_STATUS_INTERVAL_MS as f64, MAX_STATUS_INTERVAL_MS as f64)
        as u64;

    {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);

        let Some(key_index) = key_index_for_slot(&state.config, current_page, selected_slot) else {
            return false;
        };

        let key = &mut state.config.keys[key_index];
        key.action = action;
        match mode {
            EditorMode::Blank => {
                key.icon = CLOCK_BACKGROUND_ICON.to_string();
                key.clock_background = None;
                key.status = None;
                key.status_interval_ms = None;
                key.icon_on = None;
                key.icon_off = None;
            }
            EditorMode::Clock => {
                key.icon = CLOCK_ICON_ALIAS.to_string();
                let selected_bg =
                    dropdown_selected_icon(&widgets.clock_background_dropdown, clock_backgrounds);
                key.clock_background = if selected_bg == CLOCK_BACKGROUND_ICON {
                    None
                } else {
                    Some(selected_bg)
                };
                key.status = None;
                key.status_interval_ms = None;
                key.icon_on = None;
                key.icon_off = None;
            }
            EditorMode::Calendar => {
                key.icon = CALENDAR_ICON_ALIAS.to_string();
                key.clock_background = None;
                key.status = None;
                key.status_interval_ms = None;
                key.icon_on = None;
                key.icon_off = None;
            }
            EditorMode::Regular => {
                key.icon = regular_icon;
                key.clock_background = None;
                key.status = None;
                key.status_interval_ms = None;
                key.icon_on = None;
                key.icon_off = None;
            }
            EditorMode::Status => {
                key.icon = icon_off_selected.clone();
                key.clock_background = None;
                key.status = status;
                key.status_interval_ms = key.status.as_ref().map(|_| interval);
                key.icon_on = Some(icon_on_selected);
                key.icon_off = Some(icon_off_selected);
            }
        }
    }
    true
}

pub(crate) fn clear_selected_key(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    selected_slot: usize,
) -> bool {
    let mut state = state.borrow_mut();
    normalize_config(&mut state.config);
    let Some(key_index) = key_index_for_slot(&state.config, current_page, selected_slot) else {
        return false;
    };
    let is_last = key_index + 1 == state.config.keys.len();
    if is_last {
        state.config.keys.remove(key_index);
    } else {
        state.config.keys[key_index] = KeyBinding::default();
    }
    true
}

pub(crate) fn swap_keys_between_slots(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    source_slot: usize,
    target_slot: usize,
) -> bool {
    let mut state = state.borrow_mut();
    normalize_config(&mut state.config);
    let Some(source_index) = key_index_for_slot(&state.config, current_page, source_slot) else {
        return false;
    };
    let Some(target_index) = key_index_for_slot(&state.config, current_page, target_slot) else {
        return false;
    };
    if source_index == target_index {
        return true;
    }
    state.config.keys.swap(source_index, target_index);
    true
}

pub(crate) fn move_key_between_slots(
    state: &Rc<RefCell<AppState>>,
    current_page: usize,
    source_slot: usize,
    target_slot: usize,
    insert_after_target: bool,
) -> bool {
    let mut state = state.borrow_mut();
    normalize_config(&mut state.config);
    let Some(source_index) = key_index_for_slot(&state.config, current_page, source_slot) else {
        return false;
    };
    let Some(target_index) = key_index_for_slot(&state.config, current_page, target_slot) else {
        return false;
    };
    if source_index == target_index {
        return true;
    }

    let key = state.config.keys.remove(source_index);
    let mut insert_index = if insert_after_target {
        target_index.saturating_add(1)
    } else {
        target_index
    };
    if source_index < insert_index {
        insert_index = insert_index.saturating_sub(1);
    }
    insert_index = insert_index.min(state.config.keys.len());
    state.config.keys.insert(insert_index, key);
    true
}

pub(crate) fn editor_refresh_preview(
    state: &Rc<RefCell<AppState>>,
    widgets: &EditorWidgets,
    icon_names: &[String],
    clock_backgrounds: &[String],
) {
    let image_dirs = state.borrow().image_dirs.clone();
    let preview = preview_key_from_editor(widgets, icon_names, clock_backgrounds);
    set_picture_icon(
        &widgets.icon_preview,
        &image_dirs,
        &preview,
        clock_backgrounds,
    );
}

pub(crate) fn key_uses_clock(key: &KeyBinding) -> bool {
    icon_is_clock(&key.icon)
        || key.icon_on.as_deref().is_some_and(icon_is_clock)
        || key.icon_off.as_deref().is_some_and(icon_is_clock)
}

pub(crate) fn key_uses_calendar(key: &KeyBinding) -> bool {
    icon_is_calendar(&key.icon)
        || key.icon_on.as_deref().is_some_and(icon_is_calendar)
        || key.icon_off.as_deref().is_some_and(icon_is_calendar)
}

pub(crate) fn config_uses_clock(config: &Config) -> bool {
    config.keys.iter().take(KEY_COUNT).any(key_uses_clock)
}

pub(crate) fn config_uses_calendar(config: &Config) -> bool {
    config.keys.iter().take(KEY_COUNT).any(key_uses_calendar)
}

#[cfg(test)]
mod editor_tests {
    use super::*;

    fn app_state_with_key_count(key_count: usize) -> Rc<RefCell<AppState>> {
        let config = Config {
            keys: (0..key_count)
                .map(|index| KeyBinding {
                    action: Some(format!("action-{index}")),
                    ..KeyBinding::default()
                })
                .collect(),
            ..Config::default()
        };

        Rc::new(RefCell::new(AppState {
            config,
            config_path: PathBuf::new(),
            profile: "default".to_string(),
            image_dirs: Vec::new(),
            writable_image_dir: PathBuf::new(),
        }))
    }

    #[test]
    fn clear_selected_key_replaces_key_with_blank_without_shifting() {
        let state = app_state_with_key_count(KEY_COUNT + 1);
        let deleted = clear_selected_key(&state, 0, 0);
        assert!(deleted);

        let state = state.borrow();
        assert_eq!(state.config.keys.len(), KEY_COUNT + 1);
        assert!(is_plain_blank_key(&state.config.keys[0]));
        assert_eq!(state.config.keys[1].action.as_deref(), Some("action-1"));
        assert_eq!(
            state.config.keys[KEY_COUNT].action.as_deref(),
            Some("action-15")
        );
    }

    #[test]
    fn clear_selected_key_removes_last_key_in_list() {
        let state = app_state_with_key_count(KEY_COUNT);
        let deleted = clear_selected_key(&state, 0, KEY_COUNT - 1);
        assert!(deleted);

        let state = state.borrow();
        assert_eq!(state.config.keys.len(), KEY_COUNT - 1);
        assert_eq!(
            state.config.keys[KEY_COUNT - 2].action.as_deref(),
            Some("action-13")
        );
    }

    #[test]
    fn clear_selected_key_rejects_reserved_navigation_slot() {
        let state = app_state_with_key_count(KEY_COUNT + 1);
        let deleted = clear_selected_key(&state, 0, KEY_COUNT - 1);
        assert!(!deleted);
    }

    #[test]
    fn swap_keys_between_slots_swaps_configured_buttons() {
        let state = app_state_with_key_count(KEY_COUNT + 1);
        let swapped = swap_keys_between_slots(&state, 0, 0, 1);
        assert!(swapped);

        let state = state.borrow();
        assert_eq!(state.config.keys[0].action.as_deref(), Some("action-1"));
        assert_eq!(state.config.keys[1].action.as_deref(), Some("action-0"));
    }

    #[test]
    fn swap_keys_between_slots_rejects_reserved_navigation_slot() {
        let state = app_state_with_key_count(KEY_COUNT + 1);
        let swapped = swap_keys_between_slots(&state, 0, 0, KEY_COUNT - 1);
        assert!(!swapped);
    }

    #[test]
    fn move_key_between_slots_inserts_before_target() {
        let state = app_state_with_key_count(KEY_COUNT);
        let moved = move_key_between_slots(&state, 0, 0, 2, false);
        assert!(moved);

        let state = state.borrow();
        assert_eq!(state.config.keys[0].action.as_deref(), Some("action-1"));
        assert_eq!(state.config.keys[1].action.as_deref(), Some("action-0"));
        assert_eq!(state.config.keys[2].action.as_deref(), Some("action-2"));
    }

    #[test]
    fn move_key_between_slots_inserts_after_target() {
        let state = app_state_with_key_count(KEY_COUNT);
        let moved = move_key_between_slots(&state, 0, 4, 1, true);
        assert!(moved);

        let state = state.borrow();
        assert_eq!(state.config.keys[0].action.as_deref(), Some("action-0"));
        assert_eq!(state.config.keys[1].action.as_deref(), Some("action-1"));
        assert_eq!(state.config.keys[2].action.as_deref(), Some("action-4"));
    }

    #[test]
    fn move_key_between_slots_rejects_reserved_navigation_slot() {
        let state = app_state_with_key_count(KEY_COUNT + 1);
        let moved = move_key_between_slots(&state, 0, 0, KEY_COUNT - 1, false);
        assert!(!moved);
    }

    #[test]
    fn editor_mode_index_mapping_includes_blank_mode() {
        assert_eq!(editor_mode_from_index(0), EditorMode::Blank);
        assert_eq!(editor_mode_from_index(1), EditorMode::Regular);
        assert_eq!(editor_mode_from_index(2), EditorMode::Status);
        assert_eq!(editor_mode_from_index(3), EditorMode::Clock);
        assert_eq!(editor_mode_from_index(4), EditorMode::Calendar);
        assert_eq!(
            editor_mode_from_index(gtk::INVALID_LIST_POSITION),
            EditorMode::Blank
        );
    }

    #[test]
    fn blank_key_detection_distinguishes_plain_blank_from_status_blank() {
        let plain_blank = KeyBinding::default();
        assert!(is_plain_blank_key(&plain_blank));

        let status_blank = KeyBinding {
            status: Some("echo ok".to_string()),
            ..KeyBinding::default()
        };
        assert!(!is_plain_blank_key(&status_blank));
    }
}
