pub const STREAMDECK_KEY_COUNT: usize = 15;
pub const MIN_KEYS_PER_PAGE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationSlot {
    PreviousPage,
    NextPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagingLayout {
    total_slots: usize,
    keys_per_page: usize,
}

impl PagingLayout {
    pub fn new(total_slots: usize, keys_per_page: usize) -> Self {
        Self {
            total_slots,
            keys_per_page,
        }
    }

    pub fn total_slots(self) -> usize {
        self.total_slots
    }

    pub fn keys_per_page(self) -> usize {
        self.keys_per_page
    }

    pub fn edge_page_action_key_count(self) -> usize {
        self.keys_per_page.saturating_sub(1)
    }

    pub fn paged_action_key_count(self) -> usize {
        self.keys_per_page.saturating_sub(2)
    }

    pub fn previous_page_key(self) -> usize {
        self.keys_per_page.saturating_sub(2)
    }

    pub fn next_page_key(self) -> usize {
        self.keys_per_page.saturating_sub(1)
    }

    pub fn page_count(self, action_count: usize) -> usize {
        if action_count <= self.keys_per_page {
            1
        } else if action_count <= self.edge_page_action_key_count() * 2 {
            2
        } else {
            let remaining = action_count - (self.edge_page_action_key_count() * 2);
            2 + remaining.div_ceil(self.paged_action_key_count())
        }
    }

    pub fn page_capacity(self, page: usize, total_pages: usize) -> usize {
        if total_pages == 1 {
            self.keys_per_page
        } else if page == 0 || page + 1 == total_pages {
            self.edge_page_action_key_count()
        } else {
            self.paged_action_key_count()
        }
    }

    pub fn page_offset(self, page: usize, total_pages: usize) -> usize {
        (0..page)
            .map(|page_index| self.page_capacity(page_index, total_pages))
            .sum::<usize>()
    }

    pub fn navigation_slot_for_slot(
        self,
        page: usize,
        total_pages: usize,
        slot: usize,
    ) -> Option<NavigationSlot> {
        if total_pages <= 1 || slot >= self.total_slots {
            return None;
        }

        let has_prev = page > 0;
        let has_next = page + 1 < total_pages;
        let last_slot = self.next_page_key();
        let penultimate_slot = self.previous_page_key();

        if has_prev && has_next {
            if slot == penultimate_slot {
                return Some(NavigationSlot::PreviousPage);
            }
            if slot == last_slot {
                return Some(NavigationSlot::NextPage);
            }
        } else if has_prev {
            if slot == last_slot {
                return Some(NavigationSlot::PreviousPage);
            }
        } else if has_next && slot == last_slot {
            return Some(NavigationSlot::NextPage);
        }

        None
    }

    pub fn key_index_for_slot(self, action_count: usize, page: usize, slot: usize) -> Option<usize> {
        let total_pages = self.page_count(action_count);
        let page = page.min(total_pages.saturating_sub(1));
        if slot >= self.total_slots {
            return None;
        }
        if self.navigation_slot_for_slot(page, total_pages, slot).is_some() {
            return None;
        }

        let capacity = self.page_capacity(page, total_pages);
        if slot >= capacity {
            return None;
        }

        let index = self.page_offset(page, total_pages) + slot;
        (index < action_count).then_some(index)
    }

    pub fn locate_key_slot(self, action_count: usize, key_index: usize) -> Option<(usize, usize)> {
        let total_pages = self.page_count(action_count).max(1);
        for page in 0..total_pages {
            for slot in 0..self.total_slots {
                if self.key_index_for_slot(action_count, page, slot) == Some(key_index) {
                    return Some((page, slot));
                }
            }
        }
        None
    }
}
