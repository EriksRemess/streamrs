use super::open_device;
use crate::i18n::{tr, trf};
use hidapi::HidDevice;

struct UnitInfo {
    rows: u8,
    columns: u8,
    key: (u16, u16),
    screen: (u16, u16),
    image_bpp: Option<u8>,
    color_scheme: Option<u8>,
    key_images: Option<u8>,
    lcd_images: Option<u8>,
    demo_frames: Option<u8>,
}

pub struct DeviceInfo {
    product: Option<String>,
    usb_id: String,
    serial: Option<String>,
    primary: Option<FirmwareVersion>,
    loader: Option<FirmwareVersion>,
    ap1: Option<FirmwareVersion>,
    unit: Option<UnitInfo>,
    pub sleep_timer: Option<SleepTimer>,
}

pub fn open_info_device(
    vendor: u16,
    product: u16,
    usage: u16,
    usage_page: u16,
) -> Result<HidDevice, String> {
    open_device(vendor, product, usage, usage_page)?.ok_or_else(|| {
        trf(
            "No matching HID device ({id}, usage page {page}, usage {usage})",
            &[
                ("id", format!("{vendor:04x}:{product:04x}")),
                ("page", format!("{usage_page:#06x}")),
                ("usage", format!("{usage:#06x}")),
            ],
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepTimer(i32);

impl SleepTimer {
    pub fn from_minutes(minutes: u32) -> Option<Self> {
        let seconds = i32::try_from(minutes).ok()?.checked_mul(60)?;
        Some(Self(seconds))
    }

    pub fn whole_minutes(self) -> Option<u32> {
        let seconds = self.0 as u32;
        seconds.is_multiple_of(60).then_some(seconds / 60)
    }

    fn parse(data: &[u8]) -> Option<Self> {
        let [4, a, b, c, d, ..] = *data else {
            return None;
        };
        let seconds = i32::from_le_bytes([a, b, c, d]);
        (seconds >= 0).then_some(Self(seconds))
    }

    fn read(device: &HidDevice) -> Option<Self> {
        report(
            &|buf| device.get_feature_report(buf).ok(),
            0x0a,
            Self::parse,
        )
    }

    fn feature_report(self) -> [u8; 32] {
        let mut data = [0; 32];
        data[..2].copy_from_slice(&[0x03, 0x0d]);
        data[2..6].copy_from_slice(&self.0.to_le_bytes());
        data
    }

    pub fn apply(self, device: &HidDevice) -> Result<(), String> {
        let current = Self::read(device).ok_or_else(|| tr("Could not read the sleep timer."))?;
        if current == self {
            return Ok(());
        }
        device
            .send_feature_report(&self.feature_report())
            .map_err(|err| {
                trf(
                    "Could not set the sleep timer: {err}",
                    &[("err", err.to_string())],
                )
            })?;
        self.confirm(|| Self::read(device), std::thread::sleep)
    }

    fn confirm(
        self,
        mut read: impl FnMut() -> Option<Self>,
        mut wait: impl FnMut(std::time::Duration),
    ) -> Result<(), String> {
        // The MK.2 can return the previous value just after accepting a write.
        for _ in 0..10 {
            wait(std::time::Duration::from_millis(50));
            if read() == Some(self) {
                return Ok(());
            }
        }
        Err(tr("The device did not confirm the new sleep timer."))
    }
}

impl DeviceInfo {
    pub fn read(vendor: u16, product: u16, usage: u16, usage_page: u16) -> Result<Self, String> {
        let device = open_info_device(vendor, product, usage, usage_page)?;
        Ok(Self::from_device(&device, vendor, product))
    }

    pub fn from_device(device: &HidDevice, vendor: u16, product: u16) -> Self {
        let read = |buffer: &mut [u8]| device.get_feature_report(buffer).ok();
        let serial = report(&read, 0x06, |data| {
            let (&len, bytes) = data.split_first()?;
            ascii(bytes.get(..usize::from(len))?)
        })
        .or_else(|| device.get_serial_number_string().ok().flatten());
        Self {
            product: device.get_product_string().ok().flatten(),
            usb_id: format!("{vendor:04x}:{product:04x}"),
            serial,
            primary: primary_firmware(device),
            loader: report(&read, 0x04, firmware),
            ap1: report(&read, 0x07, firmware),
            unit: report(&read, 0x08, unit_info),
            sleep_timer: SleepTimer::read(device),
        }
    }

    pub fn rows(&self, tr: impl Fn(&str) -> String) -> Vec<(String, String)> {
        let unit = self.unit.as_ref();
        let number =
            |field: fn(&UnitInfo) -> Option<u8>| unit.and_then(field).map(|n| n.to_string());
        let fields = [
            (tr("Product"), self.product.clone()),
            (tr("USB ID"), Some(self.usb_id.clone())),
            (tr("Serial number"), self.serial.clone()),
            (
                tr("Primary firmware (AP2)"),
                self.primary.as_ref().map(ToString::to_string),
            ),
            (
                tr("Loader firmware (LD)"),
                self.loader.as_ref().map(ToString::to_string),
            ),
            (
                tr("Firmware (AP1)"),
                self.ap1.as_ref().map(ToString::to_string),
            ),
            (
                tr("Key layout (rows × columns)"),
                unit.map(|g| format!("{} × {}", g.rows, g.columns)),
            ),
            (
                tr("Key size"),
                unit.map(|g| format!("{} × {} px", g.key.0, g.key.1)),
            ),
            (
                tr("Display size"),
                unit.map(|g| format!("{} × {} px", g.screen.0, g.screen.1)),
            ),
            (tr("Image BPP"), number(|u| u.image_bpp)),
            (tr("Color scheme code"), number(|u| u.color_scheme)),
            (tr("Key gallery images"), number(|u| u.key_images)),
            (tr("LCD gallery images"), number(|u| u.lcd_images)),
            (tr("Demo frames"), number(|u| u.demo_frames)),
            (
                tr("Sleep timer"),
                self.sleep_timer.map(|timer| match timer.0 {
                    0 => tr("Disabled"),
                    secs => format!("{secs} s"),
                }),
            ),
        ];
        fields
            .into_iter()
            .map(|(label, value)| (label, value.unwrap_or_else(|| tr("Unavailable"))))
            .collect()
    }
}

fn report<T>(
    read: &impl Fn(&mut [u8]) -> Option<usize>,
    id: u8,
    parse: impl FnOnce(&[u8]) -> Option<T>,
) -> Option<T> {
    let mut buffer = [0; 32];
    buffer[0] = id;
    let count = read(&mut buffer)?;
    let (&returned_id, data) = buffer.get(..count)?.split_first()?;
    if returned_id != id {
        return None;
    }
    parse(data)
}

fn ascii(bytes: &[u8]) -> Option<String> {
    let text = bytes.split(|&b| b == 0).next()?;
    if text.is_empty() || !text.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        return None;
    }
    Some(std::str::from_utf8(text).ok()?.to_owned())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FirmwareVersion {
    pub(super) version: String,
    pub(super) checksum: u32,
}

impl std::fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (0x{:08x})", self.version, self.checksum)
    }
}

pub(super) fn primary_firmware(device: &HidDevice) -> Option<FirmwareVersion> {
    report(&|buf| device.get_feature_report(buf).ok(), 0x05, firmware)
}

fn firmware(data: &[u8]) -> Option<FirmwareVersion> {
    let [12, a, b, c, d, rest @ ..] = data else {
        return None;
    };
    let version = ascii(rest.get(..8)?)?;
    let checksum = u32::from_le_bytes([*a, *b, *c, *d]);
    Some(FirmwareVersion { version, checksum })
}

fn unit_info(data: &[u8]) -> Option<UnitInfo> {
    let &[
        rows,
        columns,
        kw0,
        kw1,
        kh0,
        kh1,
        sw0,
        sw1,
        sh0,
        sh1,
        ref extra @ ..,
    ] = data
    else {
        return None;
    };
    Some(UnitInfo {
        rows,
        columns,
        key: (
            u16::from_le_bytes([kw0, kw1]),
            u16::from_le_bytes([kh0, kh1]),
        ),
        screen: (
            u16::from_le_bytes([sw0, sw1]),
            u16::from_le_bytes([sh0, sh1]),
        ),
        image_bpp: extra.first().copied(),
        color_scheme: extra.get(1).copied(),
        key_images: extra.get(2).copied(),
        lcd_images: extra.get(3).copied(),
        demo_frames: extra.get(4).copied(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn firmware_checks_report_id_and_received_length() {
        let mut data = [0u8; 32];
        data[..14].copy_from_slice(b"\x05\x0c\xb2\xcd\xb1\x351.02.000");
        let query = |id, count| {
            report(
                &|buf| {
                    buf.copy_from_slice(&data);
                    Some(count)
                },
                id,
                firmware,
            )
        };
        assert_eq!(query(5, 32).unwrap().to_string(), "1.02.000 (0x35b1cdb2)");
        assert_eq!(query(5, 13), None);
        assert_eq!(query(7, 32), None);
        assert_eq!(query(5, 0), None);
    }

    #[test]
    fn sleep_timer_reports_use_signed_little_endian_seconds() {
        let timer = SleepTimer::from_minutes(10).unwrap();
        let mut expected = [0; 32];
        expected[..6].copy_from_slice(&[3, 13, 88, 2, 0, 0]);
        assert_eq!(timer.feature_report(), expected);
        assert_eq!(SleepTimer::parse(&[4, 88, 2, 0, 0]), Some(timer));
        assert_eq!(
            SleepTimer::parse(&[4, 1, 0, 0, 0]).unwrap().whole_minutes(),
            None
        );
        assert_eq!(SleepTimer::parse(&[4, 255, 255, 255, 255]), None);
        assert_eq!(SleepTimer::parse(&[4, 0, 0]), None);
        assert_eq!(SleepTimer::parse(&[3, 0, 0, 0, 0]), None);
    }

    #[test]
    fn sleep_timer_waits_for_firmware_to_apply_the_change() {
        let timer = SleepTimer(60);
        let mut reads = [Some(SleepTimer(0)), None, Some(timer)].into_iter();
        assert!(timer.confirm(|| reads.next().unwrap(), |_| {}).is_ok());
        assert!(timer.confirm(|| Some(SleepTimer(0)), |_| {}).is_err());
    }

    #[test]
    fn unit_info_preserves_present_zero_counts_and_accepts_short_reports() {
        let data = [3, 5, 72, 0, 72, 0, 224, 1, 16, 1, 4, 5, 1, 4, 4, 0];
        let unit = unit_info(&data).unwrap();
        assert_eq!(
            [
                unit.image_bpp,
                unit.color_scheme,
                unit.key_images,
                unit.lcd_images,
                unit.demo_frames
            ],
            [Some(4), Some(5), Some(1), Some(4), Some(4)]
        );
        let short = unit_info(&data[..10]).unwrap();
        assert_eq!((short.image_bpp, short.demo_frames), (None, None));
        let mut partial = data[..13].to_vec();
        partial[12] = 0;
        let partial = unit_info(&partial).unwrap();
        assert_eq!((partial.key_images, partial.lcd_images), (Some(0), None));
    }

    #[test]
    fn geometry_uses_little_endian_dimensions() {
        let info = unit_info(&[3, 5, 72, 0, 72, 0, 224, 1, 16, 1]).unwrap();
        assert_eq!(
            (info.rows, info.columns, info.key, info.screen),
            (3, 5, (72, 72), (480, 272))
        );
        assert!(unit_info(&[3, 5]).is_none());
    }
}
