use super::*;

pub(crate) fn install_css() {
    let css = r#"
.streamrs-root { padding: 12px; }
headerbar.window-titlebar {
    background: transparent;
    background-image: none;
    box-shadow: none;
    border: none;
}
.config-bar {
    padding: 10px 12px;
    border-radius: 12px;
    background: alpha(@headerbar_bg_color, 0.45);
}
.deck-card, .inspector-card {
    border-radius: 16px;
    background: alpha(@headerbar_bg_color, 0.20);
    padding: 14px;
}
.section-title { font-weight: 700; font-size: 1.04rem; margin-bottom: 8px; }
.header-title-label { font-weight: 700; }
.field-label { font-weight: 600; opacity: 0.92; margin-top: 4px; }
.status-label { opacity: 0.85; }
.status-bar {
    margin-top: 8px;
    padding: 8px 12px;
    border-radius: 10px;
    background: alpha(@headerbar_bg_color, 0.20);
}
.close-button { min-width: 34px; min-height: 34px; }
.key-button {
    background: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
}
.key-button:hover {
    background: alpha(@accent_color, 0.08);
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
    background: alpha(@view_bg_color, 0.35);
    padding: 6px;
}
.key-icon {
    border-radius: 14px;
}
.dropdown-icon {
    border-radius: 6px;
}
.icon-add-button {
    min-width: 28px;
    min-height: 28px;
    padding: 0;
}
.action-button {
    min-height: 38px;
    min-width: 0;
    border-radius: 12px;
    font-weight: 700;
    padding: 0 14px;
}
.apply-button {
    background: alpha(@accent_color, 0.80);
    color: @accent_fg_color;
}
.clear-button {
    background: alpha(#864a66, 0.45);
    color: #ffc1d5;
}
"#;

    let provider = CssProvider::new();
    provider.load_from_data(css);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
