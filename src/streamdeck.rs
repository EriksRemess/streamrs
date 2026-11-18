pub mod info;

use crate::i18n::trf;
use crate::paging::STREAMDECK_KEY_COUNT;
use hidapi::{HidApi, HidDevice, HidResult};
use std::cmp::min;

pub fn get_device(
    vendor_id: u16,
    product_id: u16,
    usage: u16,
    usage_page: u16,
) -> Option<HidDevice> {
    open_device(vendor_id, product_id, usage, usage_page).unwrap_or_else(|err| {
        eprintln!("{err}");
        None
    })
}

fn open_device(
    vendor_id: u16,
    product_id: u16,
    usage: u16,
    usage_page: u16,
) -> Result<Option<HidDevice>, String> {
    let api = HidApi::new().map_err(|err| {
        trf(
            "Could not enumerate HID devices: {err}",
            &[("err", err.to_string())],
        )
    })?;
    let mut last_error = None;
    for dev in api.device_list().filter(|dev| {
        (
            dev.vendor_id(),
            dev.product_id(),
            dev.usage(),
            dev.usage_page(),
        ) == (vendor_id, product_id, usage, usage_page)
    }) {
        match dev.open_device(&api) {
            Ok(device) => return Ok(Some(device)),
            Err(err) => {
                last_error = Some(trf(
                    "Could not open HID device {id}: {err}",
                    &[
                        ("id", format!("{vendor_id:04x}:{product_id:04x}")),
                        ("err", err.to_string()),
                    ],
                ))
            }
        }
    }
    match last_error {
        Some(err) => Err(err),
        None => Ok(None),
    }
}

pub fn set_brightness(device: &HidDevice, percentage: usize) -> Result<(), String> {
    let mut buf = [0u8; 32];
    buf[0..3].copy_from_slice(&[0x03, 0x08, percentage as u8]);
    device
        .send_feature_report(&buf)
        .map_err(|err| format!("Failed to set brightness: {err}"))?;
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub struct KeyState(u16);

impl KeyState {
    fn from_report(report: &[u8]) -> Option<Self> {
        let (header, keys) = report.split_at_checked(4)?;
        if header != [0x01, 0x00, STREAMDECK_KEY_COUNT as u8, 0x00] {
            return None;
        }

        let mut pressed = 0;
        for (index, &value) in keys.get(..STREAMDECK_KEY_COUNT)?.iter().enumerate() {
            match value {
                0 => {}
                1 => pressed |= 1 << index,
                _ => return None,
            }
        }
        Some(Self(pressed))
    }

    pub fn new_presses(&mut self, report: Option<Self>) -> impl Iterator<Item = usize> {
        let current = report.unwrap_or(*self);
        let pressed = current.0 & !self.0;
        *self = current;
        (0..STREAMDECK_KEY_COUNT).filter(move |index| pressed & (1 << index) != 0)
    }
}

pub enum InputReport {
    Empty,
    Ignored,
    Keys(KeyState),
}

impl InputReport {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match KeyState::from_report(bytes) {
            Some(keys) => Self::Keys(keys),
            None => Self::Ignored,
        }
    }
}

pub fn read_input_report(device: &HidDevice, timeout_ms: i32) -> Result<InputReport, String> {
    let mut buf = [0u8; 32];
    match device.read_timeout(&mut buf, timeout_ms) {
        Ok(0) => Ok(InputReport::Empty),
        Ok(size) => Ok(InputReport::from_bytes(&buf[..size])),
        Err(err) => {
            // SIGHUP used for config reload can interrupt a blocking HID read.
            // Treat EINTR as a transient event and keep the existing device session.
            let err_text = err.to_string();
            if err_text
                .to_ascii_lowercase()
                .contains("interrupted system call")
            {
                Ok(InputReport::Ignored)
            } else {
                Err(format!("Failed to read button state: {err}"))
            }
        }
    }
}

pub fn read_key_state(device: &HidDevice, timeout_ms: i32) -> Result<Option<KeyState>, String> {
    Ok(match read_input_report(device, timeout_ms)? {
        InputReport::Keys(keys) => Some(keys),
        InputReport::Empty | InputReport::Ignored => None,
    })
}

pub fn read_states(device: &HidDevice, timeout_ms: i32) -> Result<Option<usize>, String> {
    let report = read_key_state(device, timeout_ms)?;
    Ok(KeyState::default().new_presses(report).next())
}

pub fn set_key_image_data(device: &HidDevice, key: u8, data: &[u8]) -> Result<(), String> {
    write_key_image(key, data, |report| device.write(report))
}

fn write_key_image(
    key: u8,
    data: &[u8],
    mut write_report: impl FnMut(&[u8]) -> HidResult<usize>,
) -> Result<(), String> {
    if data.is_empty() {
        return Err(format!("Cannot upload an empty image to button {key}"));
    }
    let mut page_number = 0;
    let mut bytes_remaining = data.len();
    while bytes_remaining > 0 {
        let this_length = min(bytes_remaining, 1024 - 8);
        let bytes_sent = page_number * (1024 - 8);
        let header = [
            0x02,
            0x07,
            key,
            if this_length == bytes_remaining { 1 } else { 0 },
            (this_length & 0xFF) as u8,
            (this_length >> 8) as u8,
            (page_number & 0xFF) as u8,
            (page_number >> 8) as u8,
        ];

        let mut payload = Vec::with_capacity(1024);
        payload.extend_from_slice(&header);
        payload.extend_from_slice(&data[bytes_sent..bytes_sent + this_length]);
        payload.resize(1024, 0);
        let written = write_report(&payload)
            .map_err(|err| format!("Failed to write image to button {key}: {err}"))?;
        if written != payload.len() {
            return Err(format!(
                "Incomplete image report {page_number} for button {key}: wrote {written} of {} bytes",
                payload.len()
            ));
        }

        bytes_remaining -= this_length;
        page_number += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_image_write_stops_before_the_next_report() {
        let image = [0xab; 3 * 1016];
        let mut reports = 0;
        let result = write_key_image(4, &image, |report| {
            reports += 1;
            Ok(if reports == 2 { 512 } else { report.len() })
        });
        assert!(result.is_err());
        assert_eq!(reports, 2);
    }

    #[test]
    fn overlapping_presses_survive_quiet_reads_and_release_independently() {
        let mut state = KeyState::default();
        let mut report = [0; 4 + STREAMDECK_KEY_COUNT];
        report[..4].copy_from_slice(&[1, 0, 15, 0]);
        let mut presses = |bytes: &[u8]| {
            state
                .new_presses(KeyState::from_report(bytes))
                .collect::<Vec<_>>()
        };

        report[4] = 1;
        report[18] = 1;
        assert_eq!(presses(&report), [0, 14]);
        assert!(presses(&[]).is_empty());
        report[8] = 1;
        assert_eq!(presses(&report), [4]);
        report[4] = 0;
        assert!(presses(&report).is_empty());
        report[4] = 1;
        assert_eq!(presses(&report), [0]);
        report[4..].fill(0);
        assert!(presses(&report).is_empty());
        report[18] = 1;
        assert_eq!(presses(&report), [14]);
    }

    #[test]
    fn only_complete_key_reports_change_the_snapshot() {
        let mut report = [0; 32];
        report[..4].copy_from_slice(&[1, 0, 15, 0]);
        assert!(KeyState::from_report(&report).is_some());
        assert!(KeyState::from_report(&report[..18]).is_none());
        for (index, value) in [(0, 2), (1, 1), (2, 14), (3, 1), (4, 2)] {
            let mut other = report;
            other[index] = value;
            assert!(KeyState::from_report(&other).is_none());
        }
    }
    #[test]
    fn empty_image_does_not_report_upload_success() {
        assert!(write_key_image(0, &[], |_| panic!("no report expected")).is_err());
    }
}
