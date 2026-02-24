mod clock;
mod editor;
mod finalize;
mod management;
mod navigation;
mod primary_actions;

pub(super) use clock::wire_clock_refresh_signal;
pub(super) use editor::wire_editor_dropdown_signals;
pub(super) use finalize::finalize_and_present;
pub(super) use management::wire_management_signals;
pub(super) use navigation::wire_navigation_signals;
pub(super) use primary_actions::wire_primary_action_signals;
