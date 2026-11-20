pub mod firmware_image;
pub mod info;

use crate::i18n::trf;
use crate::paging::STREAMDECK_KEY_COUNT;
use hidapi::{HidApi, HidDevice, HidResult};
use std::borrow::Borrow;
use std::path::Path;

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
    open_matching_device(vendor_id, product_id, usage, usage_page, |hid| {
        Ok(Some(hid))
    })
}

pub fn open_daemon_display(
    vendor_id: u16,
    product_id: u16,
    usage: u16,
    usage_page: u16,
) -> Result<Option<DeckDisplay>, String> {
    open_matching_device(vendor_id, product_id, usage, usage_page, |hid| {
        let display = DeckDisplay::new(hid)?;
        #[cfg(target_os = "linux")]
        if !claim_daemon(&display.lock)
            .map_err(|err| format!("Could not claim Stream Deck: {err}"))?
        {
            return Ok(None);
        }
        Ok(Some(display))
    })
}

fn open_matching_device<T>(
    vendor_id: u16,
    product_id: u16,
    usage: u16,
    usage_page: u16,
    prepare: impl Fn(HidDevice) -> Result<Option<T>, String>,
) -> Result<Option<T>, String> {
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
            Ok(device) => match prepare(device) {
                Ok(Some(device)) => return Ok(Some(device)),
                Ok(None) => continue,
                Err(err) => last_error = Some(err),
            },
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

pub fn clear_lcd(device: &HidDevice) -> Result<(), String> {
    DeckDisplay::new(device)?.clear_lcd()
}

pub fn set_profile_brightness(
    path: &std::path::Path,
    profile: &str,
    value: u8,
) -> Result<String, String> {
    use crate::config::{current_profile::BLANK_PROFILE, streamrs_profile, toml};
    if value > 100 {
        return Err(crate::i18n::tr("Brightness must be between 0 and 100%."));
    }
    let _lock = toml::lock_for_update(path, profile == BLANK_PROFILE, std::fs::File::lock)?;
    let config = streamrs_profile::save_brightness(path, profile, value)?;
    let result = open_device(
        config.vendor_id,
        config.product_id,
        config.usage,
        config.usage_page,
    )
    .and_then(|device| {
        device
            .map(|device| set_brightness(&device, value.into()))
            .transpose()
    });
    let percent = value.to_string();
    match result {
        Ok(Some(())) => Ok(trf("Brightness set to {value}%.", &[("value", percent)])),
        Ok(None) => Ok(trf(
            "Brightness saved at {value}%. No matching device is connected.",
            &[("value", percent)],
        )),
        Err(err) => Err(trf(
            "Brightness saved at {value}%, but the device could not be updated: {err}",
            &[("value", percent), ("err", err)],
        )),
    }
}

pub fn set_brightness(device: &HidDevice, percentage: usize) -> Result<(), String> {
    let mut buf = [0u8; 32];
    buf[0..3].copy_from_slice(&[0x03, 0x08, percentage as u8]);
    device.send_feature_report(&buf).map_err(|err| {
        trf(
            "Failed to set brightness: {err}",
            &[("err", err.to_string())],
        )
    })?;
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

pub fn supports_standby_upload(vendor: u16, product: u16) -> bool {
    (vendor, product) == (0x0fd9, 0x0080)
}

pub fn set_standby_image(device: &HidDevice, path: &Path) -> Result<(), String> {
    let info = device.get_device_info().map_err(|err| err.to_string())?;
    if !supports_standby_upload(info.vendor_id(), info.product_id()) {
        return Err(crate::i18n::tr(
            "Custom standby images require a standard Stream Deck MK.2 (0fd9:0080).",
        ));
    }
    let display = DeckDisplay::new(device)?;
    let data = crate::image::wallpaper::Wallpaper::load(path)?.standby_jpeg()?;
    display.upload(ImageTarget::Standby, &data)
}

pub fn set_key_image_data(device: &HidDevice, key: u8, data: &[u8]) -> Result<(), String> {
    DeckDisplay::new(device)?.set_key_image(key, data)
}

pub fn set_panel_image_data(device: &HidDevice, data: &[u8]) -> Result<(), String> {
    DeckDisplay::new(device)?.set_panel_image(data)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayFill {
    key: Option<u8>,
    rgb: [u8; 3],
}

impl DisplayFill {
    pub fn lcd(rgb: [u8; 3]) -> Self {
        Self { key: None, rgb }
    }

    /// Key numbers are one-based, in reading order across the 15-key deck.
    pub fn key(number: u8, rgb: [u8; 3]) -> Result<Self, String> {
        if !(1..=STREAMDECK_KEY_COUNT as u8).contains(&number) {
            return Err(crate::i18n::tr("Key number must be between 1 and 15."));
        }
        Ok(Self {
            key: Some(number - 1),
            rgb,
        })
    }

    pub fn apply(self, device: &HidDevice) -> Result<(), String> {
        let info = device.get_device_info().map_err(|err| err.to_string())?;
        if info.vendor_id() != 0x0fd9
            || !matches!(info.product_id(), 0x006d | 0x0080 | 0x00a5 | 0x00b9)
        {
            return Err(crate::i18n::tr(
                "Display color fills require a supported 15-key Stream Deck.",
            ));
        }
        DeckDisplay::new(device)?.fill(self)
    }

    fn report(self) -> [u8; 32] {
        let mut report = [0; 32];
        let [r, g, b] = self.rgb;
        match self.key {
            Some(key) => report[..6].copy_from_slice(&[3, 6, key, r, g, b]),
            None => report[..5].copy_from_slice(&[3, 5, r, g, b]),
        }
        report
    }
}

#[derive(Clone, Copy)]
enum ImageTarget {
    Key(u8),
    Panel,
    Standby,
}

pub struct DeckDisplay<D = HidDevice> {
    device: D,
    #[cfg(target_os = "linux")]
    lock: std::fs::File,
}

impl<D: Borrow<HidDevice>> DeckDisplay<D> {
    pub fn new(device: D) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        let lock = {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;
            let info = device
                .borrow()
                .get_device_info()
                .map_err(|err| err.to_string())?;
            let path = Path::new(OsStr::from_bytes(info.path().to_bytes()));
            std::fs::File::options()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|err| format!("{}: {err}", path.display()))?
        };
        Ok(Self {
            device,
            #[cfg(target_os = "linux")]
            lock,
        })
    }

    pub fn hid(&self) -> &HidDevice {
        self.device.borrow()
    }

    pub fn clear_lcd(&self) -> Result<(), String> {
        self.fill(DisplayFill::lcd([0; 3]))
    }

    pub fn show_standby_image(&self) -> Result<(), String> {
        let mut report = [0; 32];
        report[..2].copy_from_slice(&[3, 2]);
        self.display_command(&report)
    }

    fn fill(&self, fill: DisplayFill) -> Result<(), String> {
        self.display_command(&fill.report())
    }

    fn display_command(&self, report: &[u8; 32]) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        let _guard = DisplayGuard::acquire(&self.lock)?;
        self.hid()
            .send_feature_report(report)
            .map_err(|err| err.to_string())
    }

    pub fn set_key_image(&self, key: u8, data: &[u8]) -> Result<(), String> {
        self.upload(ImageTarget::Key(key), data)
    }

    pub fn set_panel_image(&self, data: &[u8]) -> Result<(), String> {
        self.upload(ImageTarget::Panel, data)
    }

    fn upload(&self, target: ImageTarget, data: &[u8]) -> Result<(), String> {
        // Live images and the stored logo share a device buffer.
        #[cfg(target_os = "linux")]
        let _guard = DisplayGuard::acquire(&self.lock)?;
        write_image(target, data, |report| self.hid().write(report))
    }
}

#[cfg(target_os = "linux")]
fn claim_daemon(file: &std::fs::File) -> std::io::Result<bool> {
    use std::os::fd::AsRawFd;
    // OFD record locks last until this file description closes and are independent of flock.
    // SAFETY: Zero initializes the reserved fields and the lock starts at offset zero.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_len = 1;
    // SAFETY: The descriptor and lock structure remain valid for the call.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EAGAIN | libc::EACCES) => Ok(false),
        _ => Err(err),
    }
}

#[cfg(target_os = "linux")]
struct DisplayGuard<'a>(&'a std::fs::File);

#[cfg(target_os = "linux")]
impl<'a> DisplayGuard<'a> {
    fn acquire(file: &'a std::fs::File) -> Result<Self, String> {
        file.lock()
            .map_err(|err| format!("Could not lock image transfer: {err}"))?;
        Ok(Self(file))
    }
}

#[cfg(target_os = "linux")]
impl Drop for DisplayGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn write_image(
    target: ImageTarget,
    data: &[u8],
    mut write_report: impl FnMut(&[u8]) -> HidResult<usize>,
) -> Result<(), String> {
    if data.is_empty() {
        return Err("Cannot upload an empty image".into());
    }
    let chunks = data.chunks(1016);
    let count = chunks.len();
    if count > usize::from(u16::MAX) + 1 {
        return Err("Too many image reports".into());
    }
    for (page_number, chunk) in chunks.enumerate() {
        let page = (page_number as u16).to_le_bytes();
        let len = (chunk.len() as u16).to_le_bytes();
        let last = u8::from(page_number + 1 == count);
        let mut payload = [0; 1024];
        payload[..8].copy_from_slice(&match target {
            ImageTarget::Key(key) => [2, 7, key, last, len[0], len[1], page[0], page[1]],
            ImageTarget::Panel => [2, 8, 0, last, len[0], len[1], page[0], page[1]],
            ImageTarget::Standby => [2, 9, 0, last, page[0], page[1], len[0], len[1]],
        });
        payload[8..8 + chunk.len()].copy_from_slice(chunk);
        let written = write_report(&payload)
            .map_err(|err| format!("Failed to write image report {page_number}: {err}"))?;
        if written != payload.len() {
            return Err(format!(
                "Incomplete image report {page_number}: wrote {written} of {} bytes",
                payload.len()
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn daemon_claim_survives_independent_transfers_and_releases_on_close() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let owner = path.reopen().unwrap();
        let contender = path.reopen().unwrap();
        assert!(claim_daemon(&owner).unwrap());
        assert!(!claim_daemon(&contender).unwrap());
        let installer = path.reopen().unwrap();
        installer.try_lock().unwrap();
        drop(installer);
        assert!(!claim_daemon(&contender).unwrap());
        drop(owner);
        assert!(claim_daemon(&contender).unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_display_descriptor_serializes_transfers_after_path_removal() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let daemon = path.reopen().unwrap();
        let installer = path.reopen().unwrap();
        path.close().unwrap();

        for _ in 0..2 {
            let guard = DisplayGuard::acquire(&daemon).unwrap();
            assert!(matches!(
                installer.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            drop(guard);
            installer.try_lock().unwrap();
            installer.unlock().unwrap();
        }
    }

    #[test]
    fn rgb_fills_use_distinct_layouts_and_one_based_key_numbers() {
        let rgb = [0x12, 0x34, 0x56];
        let mut panel = [0; 32];
        panel[..5].copy_from_slice(&[3, 5, 0x12, 0x34, 0x56]);
        assert_eq!(DisplayFill::lcd(rgb).report(), panel);
        let mut key = [0; 32];
        key[..6].copy_from_slice(&[3, 6, 0, 0x12, 0x34, 0x56]);
        assert_eq!(DisplayFill::key(1, rgb).unwrap().report(), key);
        key[2] = 14;
        assert_eq!(DisplayFill::key(15, rgb).unwrap().report(), key);
        assert!(DisplayFill::key(0, rgb).is_err());
        assert!(DisplayFill::key(16, rgb).is_err());
    }

    #[test]
    fn panel_reports_use_the_reserved_byte_and_terminate_on_the_last_chunk() {
        let mut chunks = Vec::new();
        write_image(ImageTarget::Panel, &[0x5a; 1017], |report| {
            chunks.push(report.to_vec());
            Ok(report.len())
        })
        .unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(&chunks[0][..8], &[2, 8, 0, 0, 248, 3, 0, 0]);
        assert_eq!(&chunks[1][..9], &[2, 8, 0, 1, 1, 0, 1, 0, 0x5a]);
        assert_eq!(chunks[1].len(), 1024);
        assert!(chunks[1][9..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn standby_reports_put_the_chunk_index_before_the_length() {
        let mut reports = Vec::new();
        write_image(ImageTarget::Standby, &[0x5a; 1016 * 2], |report| {
            reports.push(report.to_vec());
            Ok(report.len())
        })
        .unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(&reports[0][..8], &[2, 9, 0, 0, 0, 0, 248, 3]);
        assert_eq!(&reports[1][..8], &[2, 9, 0, 1, 1, 0, 248, 3]);
        assert!(
            reports
                .iter()
                .all(|r| r.len() == 1024 && r[8..].iter().all(|&b| b == 0x5a))
        );
    }

    #[test]
    fn short_image_write_stops_before_the_next_report() {
        let image = [0xab; 3 * 1016];
        let mut reports = 0;
        let result = write_image(ImageTarget::Key(4), &image, |report| {
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
        assert!(write_image(ImageTarget::Key(0), &[], |_| panic!("no report expected")).is_err());
    }
}
