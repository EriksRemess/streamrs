impl Default for KeyBinding {
    fn default() -> Self {
        Self {
            action: None,
            icon: default_icon_name(),
            clock_background: None,
            icon_on: None,
            icon_off: None,
            status: None,
            status_interval_ms: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vendor_id: default_vendor_id(),
            product_id: default_product_id(),
            usage: default_usage(),
            usage_page: default_usage_page(),
            brightness: default_brightness(),
            keys: vec![KeyBinding::default(); KEY_COUNT],
        }
    }
}

fn normalize_config(config: &mut Config) {
    while config.keys.len() < KEY_COUNT {
        config.keys.push(KeyBinding::default());
    }
}

fn page_count(key_count: usize) -> usize {
    if key_count <= KEY_COUNT {
        1
    } else if key_count <= EDGE_PAGE_ACTION_KEY_COUNT * 2 {
        2
    } else {
        2 + (key_count - (EDGE_PAGE_ACTION_KEY_COUNT * 2) + PAGED_ACTION_KEY_COUNT - 1)
            / PAGED_ACTION_KEY_COUNT
    }
}

fn page_capacity(page: usize, total_pages: usize) -> usize {
    if total_pages == 1 {
        KEY_COUNT
    } else if page == 0 || page + 1 == total_pages {
        EDGE_PAGE_ACTION_KEY_COUNT
    } else {
        PAGED_ACTION_KEY_COUNT
    }
}

fn navigation_slot_for_slot(
    page: usize,
    total_pages: usize,
    slot: usize,
) -> Option<ReservedNavigationSlot> {
    if total_pages <= 1 || slot >= KEY_COUNT {
        return None;
    }

    let has_prev = page > 0;
    let has_next = page + 1 < total_pages;
    let last_slot = KEY_COUNT - 1;
    let penultimate_slot = KEY_COUNT - 2;

    if has_prev && has_next {
        if slot == penultimate_slot {
            return Some(ReservedNavigationSlot::PreviousPage);
        }
        if slot == last_slot {
            return Some(ReservedNavigationSlot::NextPage);
        }
    } else if has_prev {
        if slot == last_slot {
            return Some(ReservedNavigationSlot::PreviousPage);
        }
    } else if has_next && slot == last_slot {
        return Some(ReservedNavigationSlot::NextPage);
    }

    None
}

fn navigation_icon_name(slot: ReservedNavigationSlot) -> &'static str {
    match slot {
        ReservedNavigationSlot::PreviousPage => NAV_PREVIOUS_ICON,
        ReservedNavigationSlot::NextPage => NAV_NEXT_ICON,
    }
}

fn page_offset(page: usize, total_pages: usize) -> usize {
    (0..page)
        .map(|page_index| page_capacity(page_index, total_pages))
        .sum::<usize>()
}

fn key_index_for_slot(config: &Config, page: usize, slot: usize) -> Option<usize> {
    let total_pages = page_count(config.keys.len());
    let page = page.min(total_pages.saturating_sub(1));
    if slot >= KEY_COUNT {
        return None;
    }
    if navigation_slot_for_slot(page, total_pages, slot).is_some() {
        return None;
    }

    let local_slot = slot;
    let capacity = page_capacity(page, total_pages);
    if local_slot >= capacity {
        return None;
    }

    let offset = page_offset(page, total_pages);
    let index = offset + local_slot;
    if index < config.keys.len() {
        Some(index)
    } else {
        None
    }
}

fn locate_key_slot(config: &Config, key_index: usize) -> Option<(usize, usize)> {
    let total_pages = page_count(config.keys.len()).max(1);
    for page in 0..total_pages {
        for slot in 0..KEY_COUNT {
            if key_index_for_slot(config, page, slot) == Some(key_index) {
                return Some((page, slot));
            }
        }
    }
    None
}

fn first_editable_slot(config: &Config, page: usize) -> usize {
    for slot in 0..KEY_COUNT {
        if key_index_for_slot(config, page, slot).is_some() {
            return slot;
        }
    }
    0
}

fn clamp_page_and_selection(
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    selected_slot: &Rc<Cell<usize>>,
) {
    let (total_pages, page, selected_slot_fallback) = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        let total_pages = page_count(state.config.keys.len()).max(1);
        let page = current_page.get().min(total_pages.saturating_sub(1));
        let fallback = first_editable_slot(&state.config, page);
        (total_pages, page, fallback)
    };

    current_page.set(page.min(total_pages.saturating_sub(1)));
    let selected = selected_slot.get().min(KEY_COUNT.saturating_sub(1));
    let has_selected_action = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        key_index_for_slot(&state.config, page, selected).is_some()
    };
    if has_selected_action {
        selected_slot.set(selected);
    } else {
        selected_slot.set(selected_slot_fallback);
    }
}

fn refresh_page_controls(
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    prev_button: &Button,
    next_button: &Button,
    page_label: &Label,
) {
    let total_pages = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        page_count(state.config.keys.len()).max(1)
    };

    let page = current_page.get().min(total_pages.saturating_sub(1));
    current_page.set(page);
    prev_button.set_sensitive(page > 0);
    next_button.set_sensitive(page + 1 < total_pages);
    page_label.set_text(&format!("Page {}/{}", page + 1, total_pages));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_keys(count: usize) -> Config {
        let mut config = Config::default();
        config.keys = (0..count).map(|_| KeyBinding::default()).collect();
        config
    }

    #[test]
    fn page_count_matches_expected_boundaries() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(KEY_COUNT), 1);
        assert_eq!(page_count(KEY_COUNT + 1), 2);
        assert_eq!(page_count(EDGE_PAGE_ACTION_KEY_COUNT * 2), 2);
        assert_eq!(page_count((EDGE_PAGE_ACTION_KEY_COUNT * 2) + 1), 3);
        assert_eq!(page_count((EDGE_PAGE_ACTION_KEY_COUNT * 2) + PAGED_ACTION_KEY_COUNT), 3);
        assert_eq!(
            page_count((EDGE_PAGE_ACTION_KEY_COUNT * 2) + PAGED_ACTION_KEY_COUNT + 1),
            4
        );
    }

    #[test]
    fn navigation_slots_match_main_app_layout() {
        assert_eq!(navigation_slot_for_slot(0, 1, KEY_COUNT - 1), None);

        assert_eq!(
            navigation_slot_for_slot(0, 2, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::NextPage)
        );
        assert_eq!(
            navigation_slot_for_slot(1, 2, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::PreviousPage)
        );

        assert_eq!(
            navigation_slot_for_slot(1, 3, KEY_COUNT - 2),
            Some(ReservedNavigationSlot::PreviousPage)
        );
        assert_eq!(
            navigation_slot_for_slot(1, 3, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::NextPage)
        );
        assert_eq!(navigation_slot_for_slot(1, 3, 0), None);
    }

    #[test]
    fn key_index_mapping_respects_reserved_navigation_slots() {
        let config = config_with_keys(30);

        assert_eq!(page_count(config.keys.len()), 3);

        assert_eq!(key_index_for_slot(&config, 0, 0), Some(0));
        assert_eq!(key_index_for_slot(&config, 0, KEY_COUNT - 2), Some(13));
        assert_eq!(key_index_for_slot(&config, 0, KEY_COUNT - 1), None);

        assert_eq!(key_index_for_slot(&config, 1, 0), Some(14));
        assert_eq!(key_index_for_slot(&config, 1, KEY_COUNT - 3), Some(26));
        assert_eq!(key_index_for_slot(&config, 1, KEY_COUNT - 2), None);
        assert_eq!(key_index_for_slot(&config, 1, KEY_COUNT - 1), None);

        assert_eq!(key_index_for_slot(&config, 2, 0), Some(27));
        assert_eq!(key_index_for_slot(&config, 2, 1), Some(28));
        assert_eq!(key_index_for_slot(&config, 2, 2), Some(29));
        assert_eq!(key_index_for_slot(&config, 2, 3), None);
        assert_eq!(key_index_for_slot(&config, 2, KEY_COUNT - 1), None);
    }

    #[test]
    fn locate_key_slot_round_trips_each_existing_key() {
        let config = config_with_keys(36);
        for key_index in 0..config.keys.len() {
            let (page, slot) = locate_key_slot(&config, key_index)
                .unwrap_or_else(|| panic!("missing slot for key index {key_index}"));
            assert_eq!(key_index_for_slot(&config, page, slot), Some(key_index));
        }
        assert_eq!(locate_key_slot(&config, config.keys.len() + 1), None);
    }
}
