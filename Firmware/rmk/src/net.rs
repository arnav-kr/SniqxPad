use core::sync::atomic::{AtomicU8, Ordering};
use crate::hid::{send_keyboard_key, send_media_key, type_str};
use crate::set_alert;
use crate::snippets::{
    get_snippet_name, init_default_snippets, paste_all_snippets, type_snippet_index, update_snippet,
};

pub static ASSIGNED_IP: ::portable_atomic::AtomicU32 = ::portable_atomic::AtomicU32::new(0);
pub static INTERNET_STATUS: AtomicU8 = AtomicU8::new(0);
pub static FETCH_STATUS: AtomicU8 = AtomicU8::new(0);

static mut CLIENT_RX: [u8; 512] = [0u8; 512];
static mut CLIENT_TX: [u8; 256] = [0u8; 256];
static mut FETCH_BUF: [u8; 1024] = [0u8; 1024];
static mut HTTP_REQ_BUF: [u8; 512] = [0u8; 512];

static TCP_RX_BUFFER: ::static_cell::StaticCell<[u8; 512]> = ::static_cell::StaticCell::new();
static TCP_TX_BUFFER: ::static_cell::StaticCell<[u8; 512]> = ::static_cell::StaticCell::new();

pub fn get_static_config() -> ::embassy_net::StaticConfigV4 {
    let mut dns = ::heapless::Vec::new();
    let _ = dns.push(::embassy_net::Ipv4Address::new(192, 168, 7, 1));
    let _ = dns.push(::embassy_net::Ipv4Address::new(1, 1, 1, 1));
    ::embassy_net::StaticConfigV4 {
        address: ::embassy_net::Ipv4Cidr::new(
            ::embassy_net::Ipv4Address::new(192, 168, 7, 2),
            24,
        ),
        gateway: Some(::embassy_net::Ipv4Address::new(192, 168, 7, 1)),
        dns_servers: dns,
    }
}

async fn write_all(socket: &mut ::embassy_net::tcp::TcpSocket<'_>, mut buf: &[u8]) -> Result<(), ()> {
    while !buf.is_empty() {
        match socket.write(buf).await {
            Ok(0) => return Err(()),
            Ok(n) => buf = &buf[n..],
            Err(_) => return Err(()),
        }
    }
    Ok(())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn url_decode<const N: usize>(input: &str) -> heapless::String<N> {
    let mut out = heapless::String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            let _ = out.push(' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                let decoded_byte = (h1 << 4) | h2;
                let _ = out.push(decoded_byte as char);
                i += 3;
            } else {
                let _ = out.push(bytes[i] as char);
                i += 1;
            }
        } else {
            let _ = out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn get_query_param<'a>(req: &'a str, key: &str) -> Option<&'a str> {
    let query_start = req.find('?')?;
    let path_end = req[query_start..].find(' ').map(|p| query_start + p).unwrap_or(req.len());
    let query_str = &req[query_start + 1..path_end];

    for part in query_str.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return Some(v);
            }
        } else if part == key {
            return Some("");
        }
    }
    None
}

fn parse_ipv4(s: &str) -> Option<::embassy_net::Ipv4Address> {
    let mut octets = [0u8; 4];
    let mut count = 0;
    for part in s.split('.') {
        if count >= 4 {
            return None;
        }
        let val: u8 = part.parse().ok()?;
        octets[count] = val;
        count += 1;
    }
    if count == 4 {
        Some(::embassy_net::Ipv4Address::new(octets[0], octets[1], octets[2], octets[3]))
    } else {
        None
    }
}

pub async fn fetch_remote_snippets(
    url_str: &str,
    stack: ::embassy_net::Stack<'static>,
) -> Result<usize, &'static str> {
    let clean = if let Some(s) = url_str.strip_prefix("http://") {
        s
    } else if let Some(s) = url_str.strip_prefix("https://") {
        s
    } else {
        url_str
    };

    let (host_port, path) = match clean.find('/') {
        Some(idx) => (&clean[..idx], &clean[idx..]),
        None => (clean, "/"),
    };

    let (host, port) = match host_port.rfind(':') {
        Some(idx) => {
            let h = &host_port[..idx];
            let p = host_port[idx + 1..].parse::<u16>().unwrap_or(80);
            (h, p)
        }
        None => (host_port, 80),
    };

    if host.is_empty() {
        return Err("Empty host");
    }

    let ip = if let Some(ip) = parse_ipv4(host) {
        ip
    } else {
        let dns_res = ::embassy_time::with_timeout(
            ::embassy_time::Duration::from_secs(3),
            stack.dns_query(host, ::embassy_net::dns::DnsQueryType::A),
        )
        .await;
        match dns_res {
            Ok(Ok(addrs)) => {
                if let Some(::embassy_net::IpAddress::Ipv4(ipv4)) = addrs.first() {
                    *ipv4
                } else {
                    return Err("No IPv4");
                }
            }
            _ => return Err("DNS failed"),
        }
    };

    let rx = unsafe { &mut CLIENT_RX[..] };
    let tx = unsafe { &mut CLIENT_TX[..] };
    let mut socket = ::embassy_net::tcp::TcpSocket::new(stack, rx, tx);
    socket.set_timeout(Some(::embassy_time::Duration::from_secs(4)));

    let conn = ::embassy_time::with_timeout(
        ::embassy_time::Duration::from_secs(3),
        socket.connect((ip, port)),
    )
    .await;

    if conn.is_err() || conn.unwrap().is_err() {
        socket.close();
        return Err("Connect failed");
    }

    let mut req_header: heapless::String<160> = heapless::String::new();
    let _ = core::fmt::write(
        &mut req_header,
        format_args!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: SniqxPad\r\n\r\n",
            path, host
        ),
    );

    if write_all(&mut socket, req_header.as_bytes()).await.is_err() {
        socket.close();
        return Err("Write failed");
    }
    let _ = socket.flush().await;

    let fetch_buf = unsafe { &mut FETCH_BUF[..] };
    let mut total_read = 0;
    while total_read < fetch_buf.len() {
        match socket.read(&mut fetch_buf[total_read..]).await {
            Ok(0) => break,
            Ok(n) => total_read += n,
            Err(_) => break,
        }
    }
    socket.close();

    let resp_text = core::str::from_utf8(&fetch_buf[..total_read]).unwrap_or("");
    let body = match resp_text.find("\r\n\r\n") {
        Some(idx) => &resp_text[idx + 4..],
        None => resp_text,
    };

    let mut count = 0;
    init_default_snippets();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if count >= 8 {
            break;
        }
        if let Some((n, t)) = trimmed.split_once(':') {
            update_snippet(count, n.trim(), t.trim());
        } else {
            let mut auto_name: heapless::String<16> = heapless::String::new();
            let _ = core::fmt::write(&mut auto_name, format_args!("Snip {}", count + 1));
            update_snippet(count, &auto_name, trimmed);
        }
        count += 1;
    }

    if count > 0 {
        Ok(count)
    } else {
        Err("No snippets parsed")
    }
}

pub async fn handle_http_request(
    socket: &mut ::embassy_net::tcp::TcpSocket<'_>,
    req: &str,
    stack: ::embassy_net::Stack<'static>,
) {
    if let Some(act) = get_query_param(req, "act") {
        if act == "play" {
            send_media_key(0x00CD).await;
            set_alert("ACT: PLAY/PAUSE", 25);
        } else if act == "mute" {
            send_media_key(0x00E2).await;
            set_alert("ACT: MUTE", 25);
        } else if act == "volup" {
            send_media_key(0x00E9).await;
            set_alert("ACT: VOL+", 25);
        } else if act == "voldown" {
            send_media_key(0x00EA).await;
            set_alert("ACT: VOL-", 25);
        } else if act == "esc" {
            send_keyboard_key(0, 0x29).await;
            set_alert("ACT: ESC", 25);
        } else if act == "ent" {
            send_keyboard_key(0, 0x28).await;
            set_alert("ACT: ENTER", 25);
        }
    }

    if let Some(p) = get_query_param(req, "paste") {
        if let Ok(idx) = p.parse::<usize>() {
            if idx < 8 {
                type_snippet_index(idx).await;
            }
        }
    }

    if get_query_param(req, "paste_all").is_some() {
        set_alert("PASTE ALL", 35);
        paste_all_snippets().await;
    }

    if let Some(raw_text) = get_query_param(req, "type") {
        let decoded: heapless::String<96> = url_decode(raw_text);
        if !decoded.is_empty() {
            set_alert("TYPED CUSTOM", 30);
            type_str(&decoded).await;
        }
    }

    if get_query_param(req, "save_snip").is_some() {
        let idx_str = get_query_param(req, "idx").unwrap_or("0");
        let name_raw = get_query_param(req, "name").unwrap_or("Snippet");
        let text_raw = get_query_param(req, "text").unwrap_or("");
        if let Ok(idx) = idx_str.parse::<usize>() {
            if idx < 8 {
                let name_dec: heapless::String<16> = url_decode(name_raw);
                let text_dec: heapless::String<96> = url_decode(text_raw);
                update_snippet(idx, &name_dec, &text_dec);
                let mut alert: heapless::String<24> = heapless::String::new();
                let _ = core::fmt::write(&mut alert, format_args!("SAVED: SNIP {}", idx + 1));
                set_alert(&alert, 30);
            }
        }
    }

    if let Some(raw_url) = get_query_param(req, "fetch_url") {
        let decoded_url: heapless::String<128> = url_decode(raw_url);
        if !decoded_url.is_empty() {
            set_alert("FETCHING SNIPS...", 30);
            match fetch_remote_snippets(&decoded_url, stack).await {
                Ok(cnt) => {
                    let mut alert: heapless::String<24> = heapless::String::new();
                    let _ = core::fmt::write(&mut alert, format_args!("FETCHED: {} SNIPS", cnt));
                    set_alert(&alert, 35);
                }
                Err(err) => {
                    let mut alert: heapless::String<24> = heapless::String::new();
                    let _ = core::fmt::write(&mut alert, format_args!("FAIL: {}", err));
                    set_alert(&alert, 35);
                }
            }
        }
    }

    let (test_status, status_color) = if req.contains("test=1") {
        INTERNET_STATUS.store(1, Ordering::Relaxed);
        set_alert("TESTING NET...", 30);
        let dns_res = ::embassy_time::with_timeout(
            ::embassy_time::Duration::from_secs(3),
            stack.dns_query("one.one.one.one", ::embassy_net::dns::DnsQueryType::A),
        )
        .await;

        match dns_res {
            Ok(Ok(_)) => {
                INTERNET_STATUS.store(2, Ordering::Relaxed);
                set_alert("INTERNET: ONLINE", 40);
                ("ONLINE (DNS Verified)", "#238636")
            }
            _ => {
                INTERNET_STATUS.store(3, Ordering::Relaxed);
                set_alert("INTERNET: LOCAL", 40);
                ("LOCAL ONLY (No upstream)", "#d29922")
            }
        }
    } else {
        match INTERNET_STATUS.load(Ordering::Relaxed) {
            2 => ("ONLINE (DNS Verified)", "#238636"),
            3 => ("LOCAL ONLY (No upstream)", "#d29922"),
            _ => ("READY TO TEST", "#1f6feb"),
        }
    };

    let ip_val = ASSIGNED_IP.load(Ordering::Relaxed);
    let b = ip_val.to_be_bytes();
    let mut ip_str: heapless::String<20> = heapless::String::new();
    let _ = core::fmt::write(&mut ip_str, format_args!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]));

    let mut gw_str: heapless::String<20> = heapless::String::new();
    if let Some(cfg) = stack.config_v4() {
        if let Some(gw) = cfg.gateway {
            let g = gw.octets();
            let _ = core::fmt::write(&mut gw_str, format_args!("{}.{}.{}.{}", g[0], g[1], g[2], g[3]));
        } else {
            let _ = gw_str.push_str("None (Direct Link)");
        }
    } else {
        let _ = gw_str.push_str("None");
    }

    let p1 = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n",
        "<!DOCTYPE html><html><head><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
        "<title>SniqxPad Control Hub</title>",
        "<style>",
        "body{background:#0d1117;color:#c9d1d9;font-family:system-ui,-apple-system,sans-serif;text-align:center;padding:12px;margin:0;font-size:14px}",
        "h1{color:#58a6ff;margin:4px 0 8px}",
        ".badge{display:inline-block;padding:2px 8px;border-radius:10px;font-size:11px;background:#238636;color:#fff;margin-bottom:10px}",
        ".box{background:#161b22;border:1px solid #30363d;border-radius:10px;padding:12px;max-width:380px;margin:0 auto 12px;box-shadow:0 3px 10px rgba(0,0,0,0.4)}",
        "table{width:100%;font-size:13px;text-align:left;border-collapse:collapse}td,th{padding:5px 4px;border-bottom:1px solid #21262d}",
        "button{background:#1f6feb;color:#fff;border:none;border-radius:6px;padding:8px 14px;margin:3px;font-size:13px;font-weight:600;cursor:pointer}",
        "button:hover{background:#388bfd}button:active{background:#1158c7}",
        ".btn-sm{padding:4px 8px;font-size:11px;background:#238636;margin:0}",
        ".btn-all{background:#d29922;width:100%;padding:10px;font-size:14px;margin-bottom:10px}",
        ".btn-test{background:#8957e5;width:100%;padding:8px;margin-top:8px}",
        "input,select{background:#0d1117;border:1px solid #30363d;color:#c9d1d9;padding:6px;border-radius:6px;margin:3px;font-size:12px}",
        "</style></head><body>",
        "<h1>SniqxPad Hub</h1><div class=\"badge\">RMK 0.9.0 &bull; USB CDC-NCM &bull; 6 Layers</div>",
        "<div class=\"box\"><h3 style=\"margin:4px 0 8px\">Network &amp; Internet</h3><table>",
        "<tr><td>IP Address:</td><td><b>"
    );
    let _ = write_all(socket, p1.as_bytes()).await;
    let _ = write_all(socket, ip_str.as_bytes()).await;

    let p2 = "</b></td></tr><tr><td>Gateway:</td><td>";
    let _ = write_all(socket, p2.as_bytes()).await;
    let _ = write_all(socket, gw_str.as_bytes()).await;

    let p3 = "</td></tr><tr><td>Internet:</td><td><b style=\"color:";
    let _ = write_all(socket, p3.as_bytes()).await;
    let _ = write_all(socket, status_color.as_bytes()).await;
    let p3b = "\">";
    let _ = write_all(socket, p3b.as_bytes()).await;
    let _ = write_all(socket, test_status.as_bytes()).await;

    let p4 = concat!(
        "</b></td></tr></table>",
        "<a href=\"/?test=1\"><button class=\"btn-test\">Test Internet Connectivity</button></a>",
        "</div>",
        "<div class=\"box\"><h3 style=\"margin:4px 0 8px\">Snippets Hub (Layer 4)</h3>",
        "<a href=\"/?paste_all=1\"><button class=\"btn-all\">&#9654; Paste All Snippets to PC</button></a>",
        "<table><tr><th>#</th><th>Name</th><th>Action</th></tr>"
    );
    let _ = write_all(socket, p4.as_bytes()).await;

    init_default_snippets();
    for i in 0..8 {
        let name = get_snippet_name(i);
        let mut row_buf: heapless::String<160> = heapless::String::new();
        let _ = core::fmt::write(
            &mut row_buf,
            format_args!(
                "<tr><td><b>{}</b></td><td>{}</td><td><a href=\"/?paste={}\"><button class=\"btn-sm\">Type</button></a></td></tr>",
                i + 1, name, i
            ),
        );
        let _ = write_all(socket, row_buf.as_bytes()).await;
    }

    let p5 = concat!(
        "</table><div style=\"margin-top:10px\">",
        "<form action=\"/\" method=\"GET\">",
        "<input type=\"text\" name=\"fetch_url\" placeholder=\"http://host/snippets.txt\" style=\"width:68%\">",
        "<button type=\"submit\" style=\"width:24%\">Fetch</button></form>",
        "<form action=\"/\" method=\"GET\" style=\"margin-top:6px\">",
        "<input type=\"text\" name=\"type\" placeholder=\"Custom text to type...\" style=\"width:68%\">",
        "<button type=\"submit\" style=\"width:24%\">Send</button></form>",
        "<form action=\"/\" method=\"GET\" style=\"margin-top:6px\">",
        "<input type=\"hidden\" name=\"save_snip\" value=\"1\">",
        "<select name=\"idx\" style=\"width:20%\">",
        "<option value=\"0\">#1</option><option value=\"1\">#2</option><option value=\"2\">#3</option><option value=\"3\">#4</option>",
        "<option value=\"4\">#5</option><option value=\"5\">#6</option><option value=\"6\">#7</option><option value=\"7\">#8</option>",
        "</select>",
        "<input type=\"text\" name=\"name\" placeholder=\"Name\" style=\"width:30%\">",
        "<input type=\"text\" name=\"text\" placeholder=\"Snippet Text\" style=\"width:40%\">",
        "<button type=\"submit\" style=\"width:96%;margin-top:4px\">Save Snippet</button></form>",
        "</div></div>",
        "<div class=\"box\"><h3 style=\"margin:4px 0 8px\">Remote Control</h3>",
        "<p><a href=\"/?act=play\"><button>Play/Pause</button></a>",
        "<a href=\"/?act=mute\"><button>Mute</button></a></p>",
        "<p><a href=\"/?act=voldown\"><button>Vol -</button></a>",
        "<a href=\"/?act=volup\"><button>Vol +</button></a></p>",
        "<p><a href=\"/?act=esc\"><button>ESC</button></a>",
        "<a href=\"/?act=ent\"><button>ENTER</button></a></p>",
        "</div></body></html>"
    );
    let _ = write_all(socket, p5.as_bytes()).await;
    let _ = socket.flush().await;
}

pub async fn net_task(stack: ::embassy_net::Stack<'static>) -> ! {
    let rx_buffer = TCP_RX_BUFFER.init([0u8; 512]);
    let tx_buffer = TCP_TX_BUFFER.init([0u8; 512]);

    let static_cfg = get_static_config();

    loop {
        // Wait until USB CDC-NCM link is physically UP
        stack.wait_link_up().await;

        // Try DHCP now that the link is physically connected
        stack.set_config_v4(::embassy_net::ConfigV4::Dhcp(Default::default()));

        let dhcp_res = ::embassy_time::with_timeout(
            ::embassy_time::Duration::from_secs(3),
            stack.wait_config_up(),
        )
        .await;

        if dhcp_res.is_err() || !stack.is_link_up() {
            // DHCP timed out or no DHCP server, fallback to static IP
            stack.set_config_v4(::embassy_net::ConfigV4::Static(static_cfg.clone()));
        }

        if let Some(cfg) = stack.config_v4() {
            let oct = cfg.address.address().octets();
            ASSIGNED_IP.store(u32::from_be_bytes(oct), Ordering::Relaxed);
        }

        // Run web server while link is up
        while stack.is_link_up() {
            let mut socket = ::embassy_net::tcp::TcpSocket::new(stack, rx_buffer, tx_buffer);
            match ::embassy_futures::select::select(socket.accept(80), stack.wait_link_down()).await {
                ::embassy_futures::select::Either::First(Ok(())) => {
                    socket.set_timeout(Some(::embassy_time::Duration::from_secs(3)));
                    let buf = unsafe { &mut HTTP_REQ_BUF[..] };
                    if let Ok(n) = socket.read(buf).await {
                        if n > 0 {
                            let req = core::str::from_utf8(&buf[..n]).unwrap_or("");
                            handle_http_request(&mut socket, req, stack).await;
                        }
                    }
                    socket.close();
                    let _ = ::embassy_time::with_timeout(
                        ::embassy_time::Duration::from_millis(500),
                        socket.flush(),
                    )
                    .await;
                }
                ::embassy_futures::select::Either::First(Err(_)) => {
                    socket.close();
                }
                ::embassy_futures::select::Either::Second(()) => {
                    socket.close();
                    break;
                }
            }
        }

        // Link dropped: reset IP and set static config to prevent any DHCP spinning
        ASSIGNED_IP.store(0, Ordering::Relaxed);
        stack.set_config_v4(::embassy_net::ConfigV4::Static(static_cfg.clone()));
    }
}
