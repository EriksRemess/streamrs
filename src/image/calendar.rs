use chrono::{Datelike, Local, NaiveDate};

pub const CALENDAR_ICON_ALIAS: &str = "calendar.svg";
pub const CALENDAR_ICON_PREFIX: &str = "calendar://month-day";

pub fn is_calendar_icon(icon: &str) -> bool {
    icon.eq_ignore_ascii_case(CALENDAR_ICON_ALIAS)
        || icon.eq_ignore_ascii_case(CALENDAR_ICON_PREFIX)
}

pub fn current_calendar_key() -> String {
    let today = Local::now().date_naive();
    format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        today.month(),
        today.day()
    )
}

fn month_name_en(month: u32) -> &'static str {
    match month {
        1 => "JAN",
        2 => "FEB",
        3 => "MAR",
        4 => "APR",
        5 => "MAY",
        6 => "JUN",
        7 => "JUL",
        8 => "AUG",
        9 => "SEP",
        10 => "OCT",
        11 => "NOV",
        12 => "DEC",
        _ => "MON",
    }
}

pub fn render_calendar_svg_for_date(date: NaiveDate) -> String {
    let month_name = month_name_en(date.month());
    let day = date.day();
    let red_height = 22.0f32;
    let month_font_size = 12.0f32;
    let month_text_y = (red_height / 2.0) + 1.0;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
<defs>
  <clipPath id="calendar-clip">
    <rect x="0" y="0" width="72" height="72" rx="12" ry="12"/>
  </clipPath>
</defs>
<g clip-path="url(#calendar-clip)">
  <rect x="0" y="0" width="72" height="72" fill="#11111b"/>
  <rect x="0" y="0" width="72" height="22" fill="#d20f39"/>
  <rect x="0" y="22" width="72" height="50" fill="#11111b"/>
</g>
<text x="36" y="{month_text_y:.1}" text-anchor="middle" dominant-baseline="middle"
      font-family="DejaVu Sans, Arial, sans-serif" font-size="{month_font_size:.1}" font-weight="700" fill="#cdd6f4">{month_name}</text>
<text x="36" y="49" text-anchor="middle" dominant-baseline="middle"
      font-family="DejaVu Sans, Arial, sans-serif" font-size="34" font-weight="500" fill="#cdd6f4">{day}</text>
</svg>"##
    )
}

pub fn render_calendar_svg() -> String {
    render_calendar_svg_for_date(Local::now().date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::svg::load_svg_data;

    #[test]
    fn calendar_alias_and_prefix_are_case_insensitive() {
        assert!(is_calendar_icon("calendar.svg"));
        assert!(is_calendar_icon("CALENDAR.SVG"));
        assert!(is_calendar_icon("calendar://month-day"));
        assert!(is_calendar_icon("CALENDAR://MONTH-DAY"));
        assert!(!is_calendar_icon("calendar.png"));
    }

    #[test]
    fn calendar_svg_uses_english_month_name_and_day_number() {
        let date = NaiveDate::from_ymd_opt(2026, 12, 25).expect("test date should be valid");
        let svg = render_calendar_svg_for_date(date);
        assert!(svg.contains("DEC"));
        assert!(svg.contains(">25<"));
        assert!(svg.contains(r#"font-size="12.0""#));
        assert!(svg.contains(r#"y="12.0""#));
        assert!(svg.contains("fill=\"#d20f39\""));
        assert!(svg.contains("fill=\"#11111b\""));
        assert!(svg.contains("fill=\"#cdd6f4\""));
    }

    #[test]
    fn calendar_svg_uses_abbreviated_month_names() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 25).expect("test date should be valid");
        let svg = render_calendar_svg_for_date(date);
        assert!(svg.contains("SEP"));
        assert!(!svg.contains("SEPTEMBER"));
    }

    #[test]
    fn calendar_svg_renders_day_digits_into_bottom_half() {
        let date = NaiveDate::from_ymd_opt(2026, 12, 25).expect("test date should be valid");
        let svg = render_calendar_svg_for_date(date);
        let image = load_svg_data(CALENDAR_ICON_ALIAS, svg.as_bytes(), None, 72, 72)
            .expect("calendar SVG should rasterize");

        let mut light_pixels = 0usize;
        for y in 30..68 {
            for x in 10..62 {
                let p = image.get_pixel(x, y);
                if p[3] > 0 && p[0] > 140 && p[1] > 150 && p[2] > 170 {
                    light_pixels += 1;
                }
            }
        }
        assert!(
            light_pixels > 100,
            "calendar day digits should produce visible light pixels on the dark bottom area"
        );
    }
}
