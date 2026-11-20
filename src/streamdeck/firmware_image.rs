use super::info::{FirmwareVersion, primary_firmware};
use crate::i18n::{tr, trf};
use hidapi::HidDevice;
use std::{fs::OpenOptions, io::Read, path::Path};

const HEADER_SIZE: usize = 36;
// The MK.2 image transfer numbers 4 KiB blocks with a single byte.
const MAX_IMAGE_SIZE: u64 = 256 * 4096;

fn target(vendor: u16, product: u16) -> Option<&'static str> {
    match (vendor, product) {
        (0x0fd9, 0x0080) => Some("idisplay SD MK2 N9H20"),
        (0x0fd9, 0x00a5) => Some("idisplay SD MK2 SS N9H20"),
        _ => None,
    }
}

pub struct FirmwareComparison {
    installed: FirmwareVersion,
    image: FirmwareVersion,
}

impl FirmwareComparison {
    pub fn matches(&self) -> bool {
        self.installed == self.image
    }

    pub fn summary(&self) -> String {
        let installed = trf(
            "Installed AP2: {value}",
            &[("value", self.installed.to_string())],
        );
        let image = trf("Image AP2: {value}", &[("value", self.image.to_string())]);
        let status = if self.matches() {
            tr("Version and checksum match.")
        } else {
            tr("The image differs from the installed firmware.")
        };
        format!("{installed}\n{image}\n{status}")
    }
}

pub fn compare(device: &HidDevice, path: &Path) -> Result<FirmwareComparison, String> {
    let info = device.get_device_info().map_err(|e| e.to_string())?;
    let expected = target(info.vendor_id(), info.product_id())
        .ok_or_else(|| tr("Firmware image comparison requires a Stream Deck MK.2."))?;
    let image = read_image(path, expected)?;
    let installed =
        primary_firmware(device).ok_or_else(|| tr("Could not read the installed AP2 firmware."))?;
    Ok(FirmwareComparison { installed, image })
}

fn read_image(path: &Path, expected: &str) -> Result<FirmwareVersion, String> {
    let read_error = |err: std::io::Error| {
        trf(
            "Could not read firmware image '{path}': {err}",
            &[
                ("path", path.display().to_string()),
                ("err", err.to_string()),
            ],
        )
    };
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(read_error)?;
    let meta = file.metadata().map_err(read_error)?;
    if !meta.is_file() {
        return Err(tr("Select a regular firmware image file."));
    }
    if meta.len() > MAX_IMAGE_SIZE {
        return Err(tr("Firmware image exceeds the MK.2 image size limit."));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_IMAGE_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_IMAGE_SIZE {
        return Err(tr("Firmware image exceeds the MK.2 image size limit."));
    }
    parse_image(&bytes, expected)
}

fn parse_image(bytes: &[u8], expected: &str) -> Result<FirmwareVersion, String> {
    let (header, payload) = bytes
        .split_at_checked(HEADER_SIZE)
        .ok_or_else(|| tr("Firmware image header is incomplete."))?;
    let name = std::str::from_utf8(&header[..28])
        .ok()
        .map(|s| s.trim_end_matches(' '));
    if name != Some(expected) {
        return Err(tr("Firmware image target does not match this deck."));
    }
    let length = u32::from_le_bytes(header[28..32].try_into().unwrap());
    if payload.len() != length as usize {
        return Err(tr(
            "Firmware image payload length does not match its header.",
        ));
    }
    let checksum = u32::from_le_bytes(header[32..36].try_into().unwrap());
    if crc32fast::hash(payload) != checksum {
        return Err(tr("Firmware image checksum does not match its header."));
    }

    const MARKER: &[u8] = b"__APVER_";
    let mut version = None;
    for (offset, marker) in payload.windows(MARKER.len()).enumerate() {
        if marker != MARKER {
            continue;
        }
        let start = offset + MARKER.len();
        // The firmware also contains the standalone NUL-terminated marker string.
        if payload.get(start) == Some(&0) {
            continue;
        }
        let Some((digits, [0, ..])) = payload[start..].split_at_checked(8) else {
            return Err(tr("Invalid AP2 version marker in the firmware image."));
        };
        if !digits.iter().enumerate().all(|(i, &b)| match i {
            1 | 4 => b == b'.',
            _ => b.is_ascii_digit(),
        }) {
            return Err(tr("Invalid AP2 version marker in the firmware image."));
        }
        let value = std::str::from_utf8(digits).unwrap();
        if version.replace(value).is_some_and(|old| old != value) {
            return Err(tr("Conflicting AP2 version markers in the firmware image."));
        }
    }
    let version = version
        .ok_or_else(|| tr("Firmware image has no AP2 version marker."))?
        .to_owned();
    Ok(FirmwareVersion { version, checksum })
}

#[cfg(test)]
mod tests {
    use super::*;
    const STANDARD: &str = "idisplay SD MK2 N9H20";
    const SCISSOR: &str = "idisplay SD MK2 SS N9H20";

    fn image(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut header = vec![b' '; 28];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header.extend((payload.len() as u32).to_le_bytes());
        header.extend(crc32fast::hash(payload).to_le_bytes());
        header.extend(payload);
        header
    }

    #[test]
    fn validates_target_length_checksum_and_version_independently() {
        let data = b"x__APVER_1.02.004\0__APVER_1.02.004\0__APVER_\0";
        let mut bytes = image(STANDARD, data);
        let parsed = parse_image(&bytes, STANDARD).unwrap();
        assert_eq!(
            (parsed.version.as_str(), parsed.checksum),
            ("1.02.004", crc32fast::hash(data))
        );
        assert!(parse_image(&bytes, SCISSOR).is_err());
        assert!(parse_image(&bytes[..35], STANDARD).is_err());
        assert!(parse_image(&bytes[..bytes.len() - 1], STANDARD).is_err());
        bytes[32] ^= 1;
        assert!(parse_image(&bytes, STANDARD).is_err());
        for bad in [
            b"missing".as_slice(),
            b"__APVER_",
            b"__APVER_\0",
            b"__APVER_1.02.0045\0",
            b"__APVER_1.2a.004\0",
            b"__APVER_1.02.004\0__APVER_1.02.003\0",
        ] {
            assert!(parse_image(&image(STANDARD, bad), STANDARD).is_err());
        }
        assert_eq!(target(0x0fd9, 0x00a5), Some(SCISSOR));
        assert_eq!(target(0x0fd9, 0x0080), Some(STANDARD));
        assert_eq!(target(0x1234, 0x0080), None);
    }

    #[test]
    fn matching_needs_both_version_and_checksum() {
        let version = |text: &str, checksum| FirmwareVersion {
            version: text.to_owned(),
            checksum,
        };
        for (image, matches) in [
            (version("1.02.004", 4), true),
            (version("1.02.004", 3), false),
            (version("1.02.003", 4), false),
        ] {
            assert_eq!(
                FirmwareComparison {
                    installed: version("1.02.004", 4),
                    image
                }
                .matches(),
                matches
            );
        }
    }
}
