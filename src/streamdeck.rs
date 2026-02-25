use hidapi::{HidApi, HidDevice};
use std::cmp::min;

pub fn get_device(
    vendor_id: u16,
    product_id: u16,
    usage: u16,
    usage_page: u16,
) -> Option<HidDevice> {
    let api = HidApi::new().expect("Failed to create HID API");
    for dev in api.device_list() {
        if (
            dev.vendor_id(),
            dev.product_id(),
            dev.usage(),
            dev.usage_page(),
        ) == (vendor_id, product_id, usage, usage_page)
        {
            match dev.open_device(&api) {
                Ok(device) => {
                    return Some(device);
                }
                Err(e) => eprintln!("Error: {:?}", e),
            }
        }
    }
    None
}

pub fn set_brightness(device: &HidDevice, percentage: usize) -> Result<(), String> {
    let mut buf = [0u8; 32];
    buf[0..3].copy_from_slice(&[0x03, 0x08, percentage as u8]);
    device
        .send_feature_report(&buf)
        .map_err(|err| format!("Failed to set brightness: {err}"))?;
    Ok(())
}

fn get_pressed_button(buf: &[u8]) -> Option<usize> {
    buf.iter().position(|&x| x == 1)
}

pub fn read_states(device: &HidDevice, timeout_ms: i32) -> Result<Option<usize>, String> {
    let mut buf = [0u8; 32];
    buf[0] = 19;
    match device.read_timeout(&mut buf, timeout_ms) {
        Ok(size) if size > 0 => Ok(get_pressed_button(&buf[4..19])),
        Ok(_) => Ok(None),
        Err(err) => Err(format!("Failed to read key state: {err}")),
    }
}

pub fn set_key_image_data(device: &HidDevice, key: u8, data: &[u8]) -> Result<(), String> {
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
        device
            .write(&payload)
            .map_err(|err| format!("Failed to write image to key {key}: {err}"))?;

        bytes_remaining -= this_length;
        page_number += 1;
    }

    Ok(())
}
