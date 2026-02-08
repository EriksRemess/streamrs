use hidapi::{HidApi, HidDevice};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType::Lanczos3;
use image::imageops::{crop_imm, resize, rotate180};
use image::{load_from_memory, GenericImageView};
use std::cmp::min;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

const ACTIONS: [&str; 15] = [
    "open https://www.duckduckgo.com",
    "open https://www.youtube.com",
    "open https://www.reddit.com",
    "open https://www.github.com",
    "amixer set Capture toggle",
    "open https://www.twitter.com",
    "open https://www.instagram.com",
    "open https://www.facebook.com",
    "open https://chatgpt.com/",
    "desklight",
    "open https://www.twitch.com",
    "open https://discord.com/channels/@me",
    "open https://business.facebook.com",
    "slack",
    "ghostty",
];

const ACTION_ICONS: [&[u8]; 15] = [
    include_bytes!("../images/duckduckgo.png"),
    include_bytes!("../images/youtube.png"),
    include_bytes!("../images/reddit.png"),
    include_bytes!("../images/github.png"),
    include_bytes!("../images/zoom-unmute.png"),
    include_bytes!("../images/twitter.png"),
    include_bytes!("../images/instagram.png"),
    include_bytes!("../images/facebook.png"),
    include_bytes!("../images/chatgpt.png"),
    include_bytes!("../images/floor-lamp.png"),
    include_bytes!("../images/twitch.png"),
    include_bytes!("../images/discord.png"),
    include_bytes!("../images/meta.png"),
    include_bytes!("../images/slack.png"),
    include_bytes!("../images/ghostty.png"),
];

fn get_device(vendor_id: u16, product_id: u16, usage: u16, usage_page: u16) -> Option<HidDevice> {
    let api = HidApi::new().expect("Failed to create HID API");
    for dev in api.device_list() {
        if (
            dev.vendor_id(),
            dev.product_id(),
            dev.usage(),
            dev.usage_page(),
        ) == (vendor_id, product_id, usage, usage_page)
        {
            if let Ok(device) = dev.open_device(&api) {
                return Some(device);
            }
        }
    }
    eprintln!("Device not found");
    return None;
}

fn set_brightness(device: &HidDevice, percentage: usize) {
    let mut buf = [0u8; 32];
    buf[0..3].copy_from_slice(&[0x03, 0x08, percentage as u8]);
    device.send_feature_report(&mut buf).unwrap();
}

fn launch_app(action: &str, debug: bool) {
    let path: Vec<&str> = action.split_whitespace().collect();
    let mut cmd = Command::new(&path[0]);
    cmd.args(&path[1..]).stdin(Stdio::null());

    if debug {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    let child = cmd.spawn();
    if let Err(e) = child {
        eprintln!("Error: {:?}", e);
    }
}

fn get_pressed_button(buf: &[u8], debug: bool) {
    if let Some(index) = buf.iter().position(|&x| x == 1) {
        launch_app(ACTIONS[index as usize], debug);
    }
}

fn read_states(device: &HidDevice, debug: bool) {
    let mut buf = [0u8; 32];
    buf[0] = 19;
    if let Ok(_size) = device.read(&mut buf) {
        get_pressed_button(&buf[4..19], debug);
    }
}

fn set_key_image(device: &HidDevice, key: u8) {
    let img_data = ACTION_ICONS[key as usize];
    let img = get_image_data(img_data);
    let mut page_number = 0;
    let mut bytes_remaining = img.len();
    while bytes_remaining > 0 {
        let this_length = min(bytes_remaining, 1024 - 8);
        let bytes_sent = page_number * (1024 - 8);
        let header = [
            0x02,
            0x07,
            key as u8,
            if this_length == bytes_remaining { 1 } else { 0 },
            (this_length & 0xFF) as u8,
            (this_length >> 8) as u8,
            (page_number & 0xFF) as u8,
            (page_number >> 8) as u8,
        ];
        let mut payload = Vec::with_capacity(1024);
        payload.extend_from_slice(&header);
        payload.extend_from_slice(&img[bytes_sent..bytes_sent + this_length]);
        payload.resize(1024, 0);
        device.write(&payload).unwrap();
        bytes_remaining -= this_length;
        page_number += 1;
    }
}

fn get_image_data(img_data: &[u8]) -> Vec<u8> {
    let img = load_from_memory(img_data).unwrap();
    let (width, height) = img.dimensions();
    let crop_size = min(width, height);
    let x_offset = (width - crop_size) / 2;
    let y_offset = (height - crop_size) / 2;
    let mut img = crop_imm(&img, x_offset, y_offset, crop_size, crop_size).to_image();
    img = resize(&rotate180(&img), 72, 72, Lanczos3);
    let mut data = Vec::new();
    JpegEncoder::new_with_quality(&mut data, 100)
        .encode_image(&img)
        .unwrap();
    data
}

fn main() {
    let debug = std::env::args().any(|arg| arg == "--debug");
    if let Some(device) = get_device(0x0fd9, 0x0080, 0x0001, 0x000c) {
        set_brightness(&device, 60);
        for i in 0..15 {
            set_key_image(&device, i);
        }
        loop {
            read_states(&device, debug);
            sleep(Duration::from_millis(100));
        }
    };
}
