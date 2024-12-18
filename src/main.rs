use hidapi::{HidApi, HidDevice};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType::Lanczos3;
use image::imageops::{crop_imm, resize, rotate180};
use image::{load_from_memory, GenericImageView};
use std::cmp::min;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

const ACTIONS: [&str; 15] = [
    "open https://www.google.com",
    "open https://www.youtube.com",
    "open https://www.reddit.com",
    "open https://www.github.com",
    "amixer set Capture toggle",
    "open https://www.twitter.com",
    "open https://www.instagram.com",
    "open https://www.facebook.com",
    "open https://www.amazon.com",
    "desklight",
    "open https://www.netflix.com",
    "/opt/microsoft/msedge/microsoft-edge --profile-directory=Default --app-id=cinhimbnkkaeohfgghhklpknlkffjgod '--app-url=https://music.youtube.com/?source=pwa'",
    "open https://www.twitch.com",
    "/opt/microsoft/msedge/microsoft-edge --profile-directory=Default --app-id=cifhbcnohmdccbgoicgdjpfamggdegmo '--app-url=https://teams.microsoft.com/v2/?clientType=pwa'",
    "flatpak run com.raggesilver.BlackBox",
];

const ACTION_ICONS: [&[u8]; 15] = [
    include_bytes!("../images/google.png"),
    include_bytes!("../images/youtube.png"),
    include_bytes!("../images/reddit.png"),
    include_bytes!("../images/github.png"),
    include_bytes!("../images/zoom-unmute.png"),
    include_bytes!("../images/twitter.png"),
    include_bytes!("../images/instagram.png"),
    include_bytes!("../images/facebook.png"),
    include_bytes!("../images/amazon.png"),
    include_bytes!("../images/floor-lamp.png"),
    include_bytes!("../images/netflix.png"),
    include_bytes!("../images/youtube-music.png"),
    include_bytes!("../images/twitch.png"),
    include_bytes!("../images/teams.png"),
    include_bytes!("../images/terminal.png"),
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
            match dev.open_device(&api) {
                Ok(device) => {
                  return Some(device);
                },
                Err(e) => eprintln!("Error: {:?}", e),
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

fn launch_app(action: &str) {
    let path: Vec<&str> = action.split_whitespace().collect();
    let child = Command::new(&path[0]).args(&path[1..]).spawn();
    if let Err(e) = child {
        eprintln!("Error: {:?}", e);
    }
}

fn get_pressed_button(buf: &[u8]) {
    if let Some(index) = buf.iter().position(|&x| x == 1) {
        launch_app(ACTIONS[index as usize]);
    }
}

fn read_states(device: &HidDevice) {
    let mut buf = [0u8; 32];
    buf[0] = 19;
    if let Ok(_size) = device.read(&mut buf) {
        get_pressed_button(&buf[4..19]);
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
    ctrlc::set_handler(move || {
      println!("\nExiting...");
      std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
    if let Some(device) = get_device(0x0fd9, 0x0080, 0x0001, 0x000c) {
        set_brightness(&device, 60);
        for i in 0..15 {
            set_key_image(&device, i);
        }
        loop {
            read_states(&device);
            sleep(Duration::from_millis(100));
        }
    };
}
