use super::*;

pub(crate) fn install_css() {
    let css_template = r#"
.streamrs-root { padding: $SPACINGpx; }
headerbar.window-titlebar {
    background: transparent;
    background-image: none;
    box-shadow: none;
    border: none;
}
.deck-card {
    border-radius: 16px;
    background: alpha(@headerbar_bg_color, 0.20);
    padding: $SPACINGpx;
}
.inspector-panel {
    padding-top: 0;
}
.section-title { font-weight: 700; font-size: 1.04rem; margin-bottom: $SPACINGpx; }
.page-indicator {
    font-weight: 700;
    font-size: 1.04rem;
    opacity: 0.92;
    margin-bottom: $SPACINGpx;
}
.header-title-label { font-weight: 700; }
.field-label { font-weight: 600; opacity: 0.92; margin-top: 0; }
.status-label { opacity: 0.85; }
.status-bar {
    margin-top: 0;
    padding: $SPACINGpx;
    border-radius: 10px;
    background: alpha(@headerbar_bg_color, 0.20);
}
.main-split > separator {
    background: transparent;
    box-shadow: none;
    min-width: 10px;
    min-height: 10px;
}
.main-split.horizontal > separator {
    margin: $SPACINGpx 0;
    border-radius: 5px;
}
.main-split > separator:hover {
    background: alpha(@theme_fg_color, 0.20);
}
.main-split > separator:active {
    background: alpha(@accent_color, 0.35);
}
.inspector-scroller scrollbar.vertical,
.inspector-scroller scrollbar.vertical slider {
    margin-right: -10px;
    min-width: 4px;
}
.inspector-scroller scrollbar.vertical slider:hover {
    margin-right: -14px;
    min-width: 6px;
}
.close-button { min-width: $CONTROLpx; min-height: $CONTROLpx; }
.key-button {
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
}
.key-button:hover {
    background: alpha(@accent_color, 0.08);
}
.key-blank-binding {
    background: alpha(@theme_fg_color, 0.04);
    box-shadow:
        inset 0 0 0 2px alpha(@theme_fg_color, 0.45),
        inset 0 0 0 6px alpha(@theme_fg_color, 0.10);
    border-radius: 18px;
}
.key-empty-slot {
    opacity: 0.55;
}
.key-selected {
    outline: 2px solid @accent_color;
    outline-offset: 0;
    border-radius: 18px;
}
.key-drop-swap {
    outline: 2px solid alpha(@accent_color, 0.95);
    outline-offset: 0;
    border-radius: 18px;
}
.key-drop-before {
    box-shadow: inset 3px 0 0 0 alpha(@accent_color, 0.95);
    border-radius: 18px;
}
.key-drop-after {
    box-shadow: inset -3px 0 0 0 alpha(@accent_color, 0.95);
    border-radius: 18px;
}
.deck-image {
    border-radius: 26px;
}
.icon-preview {
    border-radius: 14px;
    background: transparent;
    padding: 0;
}
.key-icon {
    border-radius: 14px;
}
.dropdown-icon {
    border-radius: 6px;
}
.icon-add-button {
    min-width: $CONTROLpx;
    min-height: $CONTROLpx;
    padding: 0;
}
.icon-add-button > label {
    margin: 0;
    padding: 0;
    font-size: 1.1rem;
    font-weight: 700;
}
.profile-action-button {
    min-height: $CONTROLpx;
    min-width: 0;
    border-radius: 10px;
    padding: 0 10px;
}
"#;
    let css = css_template
        .replace("$SPACING", &UI_SPACING.to_string())
        .replace("$CONTROL", &UI_CONTROL_HEIGHT.to_string());

    let provider = CssProvider::new();
    provider.load_from_data(&css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
