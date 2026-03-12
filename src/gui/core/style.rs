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
.config-bar {
    padding: $SPACINGpx;
    border-radius: 12px;
    background: alpha(@headerbar_bg_color, 0.45);
}
.deck-card, .inspector-card {
    border-radius: 16px;
    background: alpha(@headerbar_bg_color, 0.20);
    padding: $SPACINGpx;
}
.inspector-card {
    padding-right: 0;
}
.inspector-card .streamrs-field {
    margin-right: $SPACINGpx;
}
.inspector-card .icon-row .streamrs-field {
    margin-right: 0;
}
.icon-row {
    margin-right: $SPACINGpx;
}
.icon-row .icon-add-button {
    margin-right: 0;
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
    min-width: 8px;
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
.streamrs-field {
    min-height: $CONTROLpx;
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
.action-button {
    min-height: $CONTROLpx;
    min-width: 0;
    border-radius: 12px;
    font-weight: 700;
    padding: 0 12px;
}
.profile-action-button {
    min-height: $CONTROLpx;
    min-width: 0;
    border-radius: 10px;
    padding: 0 10px;
}
.apply-button {
    background: alpha(@accent_color, 0.80);
    color: @accent_fg_color;
}
.clear-button {
    background: alpha(#864a66, 0.45);
    color: #ffc1d5;
}
.about-sheet {
    background: @window_bg_color;
    border-radius: 18px;
    border: 1px solid alpha(@theme_fg_color, 0.10);
}
.about-close-button {
    min-width: 34px;
    min-height: 34px;
    padding: 0;
    border-radius: 999px;
    background: alpha(@theme_fg_color, 0.14);
}
.about-close-button > image {
    -gtk-icon-size: 14px;
}
.about-close-button:hover {
    background: alpha(@theme_fg_color, 0.24);
}
.about-close-button:active {
    background: alpha(@theme_fg_color, 0.32);
}
.about-hero {
    margin-top: 2px;
    margin-bottom: 2px;
}
.about-logo {
    margin-bottom: 6px;
}
.about-title {
    font-size: 1.95rem;
    font-weight: 800;
}
.about-subtitle {
    opacity: 0.84;
    font-size: 0.95rem;
}
.about-version-pill {
    min-height: 34px;
    min-width: 0;
    border-radius: 999px;
    background: #33415f;
    border: 1px solid alpha(#9ecbff, 0.22);
    color: #9ecbff;
    font-weight: 700;
    padding: 0 14px;
    margin-top: 10px;
    font-size: 0.80rem;
}
.about-version-pill:hover {
    background: #3b4a6d;
}
.about-version-pill:active {
    background: #44547a;
}
.about-links {
    margin-top: 8px;
}
.about-link-row {
    min-height: 56px;
    border-radius: 16px;
    padding: 0 18px;
    background: alpha(@headerbar_bg_color, 0.42);
    border: 1px solid alpha(@theme_fg_color, 0.06);
}
.about-link-row:hover {
    background: alpha(@accent_color, 0.20);
    border-color: alpha(@accent_color, 0.40);
}
.about-link-row:active {
    background: alpha(@accent_color, 0.28);
    border-color: alpha(@accent_color, 0.50);
}
.about-link-label {
    font-size: 0.98rem;
    font-weight: 500;
}
.about-link-icon {
    opacity: 0.86;
    min-width: 20px;
    min-height: 20px;
}
.about-link-row:hover .about-link-icon {
    opacity: 1.0;
}
.about-link-icon-external {
    min-width: 18px;
    min-height: 18px;
}
.about-link-icon-chevron {
    min-width: 20px;
    min-height: 20px;
}
.about-link-group {
    border-radius: 16px;
    background: alpha(@headerbar_bg_color, 0.42);
    border: 1px solid alpha(@theme_fg_color, 0.06);
}
.about-link-group-separator {
    margin: 0;
    min-height: 1px;
    background: alpha(@theme_fg_color, 0.10);
}
.about-link-row.about-link-row-grouped {
    border: none;
    background: transparent;
    border-radius: 0;
}
.about-link-row.about-link-row-grouped:hover {
    background: alpha(@accent_color, 0.16);
    border-color: transparent;
}
.about-link-row.about-link-row-grouped:active {
    background: alpha(@accent_color, 0.24);
    border-color: transparent;
}
.about-link-row.about-link-row-group-top {
    border-top-left-radius: 16px;
    border-top-right-radius: 16px;
}
.about-link-row.about-link-row-group-bottom {
    border-bottom-left-radius: 16px;
    border-bottom-right-radius: 16px;
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
