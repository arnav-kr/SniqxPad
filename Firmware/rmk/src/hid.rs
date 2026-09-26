pub async fn send_media_key(usage_id: u16) {
    let report = ::rmk::hid::Report::MediaKeyboardReport(
        ::usbd_hid::descriptor::MediaKeyboardReport { usage_id },
    );
    ::rmk::channel::USB_REPORT_CHANNEL.send(report).await;
    ::embassy_time::Timer::after(::embassy_time::Duration::from_millis(20)).await;
    let release = ::rmk::hid::Report::MediaKeyboardReport(
        ::usbd_hid::descriptor::MediaKeyboardReport { usage_id: 0 },
    );
    ::rmk::channel::USB_REPORT_CHANNEL.send(release).await;
    ::embassy_time::Timer::after(::embassy_time::Duration::from_millis(10)).await;
}

pub async fn send_keyboard_key(modifier: u8, keycode: u8) {
    let mut report = ::rmk::hid::KeyboardReport {
        modifier,
        ..Default::default()
    };
    report.keycodes[0] = keycode;
    ::rmk::channel::USB_REPORT_CHANNEL.send(::rmk::hid::Report::KeyboardReport(report)).await;
    ::embassy_time::Timer::after(::embassy_time::Duration::from_millis(15)).await;
    ::rmk::channel::USB_REPORT_CHANNEL.send(::rmk::hid::Report::KeyboardReport(
        ::rmk::hid::KeyboardReport::default(),
    )).await;
    ::embassy_time::Timer::after(::embassy_time::Duration::from_millis(10)).await;
}

pub fn char_to_hid(c: char) -> Option<(u8, u8)> {
    match c {
        'a'..='z' => Some((0, 0x04 + (c as u8 - b'a'))),
        'A'..='Z' => Some((0x02, 0x04 + (c as u8 - b'A'))),
        '1'..='9' => Some((0, 0x1E + (c as u8 - b'1'))),
        '0' => Some((0, 0x27)),
        '\n' | '\r' => Some((0, 0x28)),
        '\x1b' => Some((0, 0x29)),
        '\t' => Some((0, 0x2B)),
        ' ' => Some((0, 0x2C)),
        '-' => Some((0, 0x2D)),
        '_' => Some((0x02, 0x2D)),
        '=' => Some((0, 0x2E)),
        '+' => Some((0x02, 0x2E)),
        '[' => Some((0, 0x2F)),
        '{' => Some((0x02, 0x2F)),
        ']' => Some((0, 0x30)),
        '}' => Some((0x02, 0x30)),
        '\\' => Some((0, 0x31)),
        '|' => Some((0x02, 0x31)),
        ';' => Some((0, 0x33)),
        ':' => Some((0x02, 0x33)),
        '\'' => Some((0, 0x34)),
        '"' => Some((0x02, 0x34)),
        '`' => Some((0, 0x35)),
        '~' => Some((0x02, 0x35)),
        ',' => Some((0, 0x36)),
        '<' => Some((0x02, 0x36)),
        '.' => Some((0, 0x37)),
        '>' => Some((0x02, 0x37)),
        '/' => Some((0, 0x38)),
        '?' => Some((0x02, 0x38)),
        '!' => Some((0x02, 0x1E)),
        '@' => Some((0x02, 0x1F)),
        '#' => Some((0x02, 0x20)),
        '$' => Some((0x02, 0x21)),
        '%' => Some((0x02, 0x22)),
        '^' => Some((0x02, 0x23)),
        '&' => Some((0x02, 0x24)),
        '*' => Some((0x02, 0x25)),
        '(' => Some((0x02, 0x26)),
        ')' => Some((0x02, 0x27)),
        _ => None,
    }
}

pub async fn type_str(s: &str) {
    for c in s.chars() {
        if let Some((modi, key)) = char_to_hid(c) {
            send_keyboard_key(modi, key).await;
        }
    }
}
