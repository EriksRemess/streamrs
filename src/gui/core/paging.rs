use super::*;

pub(crate) fn normalize_config(config: &mut Config) {
    config.keys_per_page = config
        .keys_per_page
        .clamp(streamrs::paging::MIN_KEYS_PER_PAGE, KEY_COUNT);
    while config.keys.len() < config.keys_per_page {
        config.keys.push(KeyBinding::default());
    }
}

pub(crate) fn paging_layout(config: &Config) -> PagingLayout {
    PagingLayout::new(KEY_COUNT, config.keys_per_page)
}

pub(crate) fn page_count(config: &Config) -> usize {
    paging_layout(config).page_count(config.keys.len())
}

pub(crate) fn navigation_slot_for_slot(
    config: &Config,
    page: usize,
    total_pages: usize,
    slot: usize,
) -> Option<ReservedNavigationSlot> {
    paging_layout(config).navigation_slot_for_slot(page, total_pages, slot)
}

pub(crate) fn navigation_icon_name(slot: ReservedNavigationSlot) -> &'static str {
    match slot {
        ReservedNavigationSlot::PreviousPage => NAV_PREVIOUS_ICON,
        ReservedNavigationSlot::NextPage => NAV_NEXT_ICON,
    }
}

pub(crate) fn key_index_for_slot(config: &Config, page: usize, slot: usize) -> Option<usize> {
    paging_layout(config).key_index_for_slot(config.keys.len(), page, slot)
}

pub(crate) fn locate_key_slot(config: &Config, key_index: usize) -> Option<(usize, usize)> {
    paging_layout(config).locate_key_slot(config.keys.len(), key_index)
}

pub(crate) fn first_editable_slot(config: &Config, page: usize) -> usize {
    for slot in 0..KEY_COUNT {
        if key_index_for_slot(config, page, slot).is_some() {
            return slot;
        }
    }
    0
}

pub(crate) fn clamp_page_and_selection(
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    selected_slot: &Rc<Cell<usize>>,
) {
    let (total_pages, page, selected_slot_fallback) = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        let total_pages = page_count(&state.config).max(1);
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

pub(crate) fn refresh_page_controls(
    state: &Rc<RefCell<AppState>>,
    current_page: &Rc<Cell<usize>>,
    prev_button: &Button,
    next_button: &Button,
    page_label: &Label,
) {
    let total_pages = {
        let mut state = state.borrow_mut();
        normalize_config(&mut state.config);
        page_count(&state.config).max(1)
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
        Config {
            keys: (0..count).map(|_| KeyBinding::default()).collect(),
            ..Config::default()
        }
    }

    #[test]
    fn page_count_matches_expected_boundaries() {
        let layout = PagingLayout::new(KEY_COUNT, KEY_COUNT);
        assert_eq!(layout.page_count(0), 1);
        assert_eq!(layout.page_count(KEY_COUNT), 1);
        assert_eq!(layout.page_count(KEY_COUNT + 1), 2);
        assert_eq!(layout.page_count((KEY_COUNT - 1) * 2), 2);
        assert_eq!(layout.page_count(((KEY_COUNT - 1) * 2) + 1), 3);
        assert_eq!(
            layout.page_count(((KEY_COUNT - 1) * 2) + (KEY_COUNT - 2)),
            3
        );
        assert_eq!(
            layout.page_count(((KEY_COUNT - 1) * 2) + (KEY_COUNT - 2) + 1),
            4
        );
    }

    #[test]
    fn navigation_slots_match_main_app_layout() {
        let config = Config::default();
        assert_eq!(navigation_slot_for_slot(&config, 0, 1, KEY_COUNT - 1), None);

        assert_eq!(
            navigation_slot_for_slot(&config, 0, 2, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::NextPage)
        );
        assert_eq!(
            navigation_slot_for_slot(&config, 1, 2, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::PreviousPage)
        );

        assert_eq!(
            navigation_slot_for_slot(&config, 1, 3, KEY_COUNT - 2),
            Some(ReservedNavigationSlot::PreviousPage)
        );
        assert_eq!(
            navigation_slot_for_slot(&config, 1, 3, KEY_COUNT - 1),
            Some(ReservedNavigationSlot::NextPage)
        );
        assert_eq!(navigation_slot_for_slot(&config, 1, 3, 0), None);
    }

    #[test]
    fn key_index_mapping_respects_reserved_navigation_slots() {
        let config = config_with_keys(30);

        assert_eq!(page_count(&config), 3);

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
