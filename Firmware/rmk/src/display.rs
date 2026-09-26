use core::sync::atomic::{AtomicU8, Ordering};
use embedded_graphics::{
    image::{Image, ImageRaw},
    mono_font::{
        ascii::{FONT_4X6, FONT_5X8, FONT_6X10, FONT_7X13_BOLD},
        MonoTextStyle,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};
use rmk::display::{DisplayRenderer, RenderContext};

use crate::bongo::{draw_rmk_logo, BONGO_BLINK, BONGO_IDLE, BONGO_TAP0, BONGO_TAP1};
use crate::net::{ASSIGNED_IP, INTERNET_STATUS};
use crate::snippets::{get_snippet_name, get_snippet_text, SELECTED_SNIPPET};

pub static ENCODER_ACTION: AtomicU8 = AtomicU8::new(0);
pub static IS_BOOTING: AtomicU8 = AtomicU8::new(0);

static ALERT_TICKS: AtomicU8 = AtomicU8::new(0);
static ALERT_BUF: ::rmk::channel::blocking_mutex::Mutex<
    ::rmk::channel::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<heapless::String<24>>,
> = ::rmk::channel::blocking_mutex::Mutex::new(core::cell::RefCell::new(heapless::String::new()));

pub fn set_alert(msg: &str, ticks: u8) {
    ALERT_BUF.lock(|cell| {
        let mut b = cell.borrow_mut();
        b.clear();
        let _ = b.push_str(msg);
    });
    ALERT_TICKS.store(ticks, Ordering::Relaxed);
}

#[derive(Default)]
pub struct SniqxRenderer {
    tap_frame: usize,
    tap_hold_ticks: u8,
    idle_ticks: u16,
    last_base_layer: u8,
    preview_ticks: u8,
    knob_indicator: Option<&'static str>,
    knob_indicator_ticks: u8,
}

struct LayerLayout {
    title: &'static str,
    knob: &'static str,
    top_key: &'static str,
    r1: [&'static str; 4],
    r2: [&'static str; 4],
}

const LAYOUTS: [LayerLayout; 5] = [
    // Layer 0: BASE
    LayerLayout {
        title: "BASE",
        knob: "S+/S-",
        top_key: "P/PK",
        r1: ["ESC", "D", "I", "ENT"],
        r2: ["H", "J", "K", "L"],
    },
    // Layer 1: GAME
    LayerLayout {
        title: "GAME",
        knob: "S+/S-",
        top_key: "ESC/PK",
        r1: ["SFT", "Q", "W", "E"],
        r2: ["CTL", "A", "S", "D"],
    },
    // Layer 2: MEDIA
    LayerLayout {
        title: "MEDIA",
        knob: "V+/V-",
        top_key: "PEEK",
        r1: ["PLAY", "PREV", "NEXT", "VOL+"],
        r2: ["MUTE", "-", "-", "VOL-"],
    },
    // Layer 3: TOOLS
    LayerLayout {
        title: "TOOLS",
        knob: "B+/B-",
        top_key: "PEEK",
        r1: ["-", "-", "-", "-"],
        r2: ["-", "-", "-", "BOOT"],
    },
    // Layer 4: SNIPPETS
    LayerLayout {
        title: "SNIP",
        knob: "RE:LIST",
        top_key: "PASTE",
        r1: ["SN1", "SN2", "SN3", "SN4"],
        r2: ["SN5", "SN6", "SN7", "ALL"],
    },
];

fn draw_snippets_browser<D: DrawTarget<Color = BinaryColor>>(display: &mut D) {
    let box_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

    // Row 0: Header
    Rectangle::new(Point::new(0, 0), Size::new(45, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    Text::new("SNIP", Point::new(14, 7), text_style).draw(display).ok();

    Rectangle::new(Point::new(47, 0), Size::new(40, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    Text::new("RE:NAV", Point::new(51, 7), text_style).draw(display).ok();

    Rectangle::new(Point::new(89, 0), Size::new(38, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    Text::new("PST:KNB", Point::new(92, 7), text_style).draw(display).ok();

    let curr = SELECTED_SNIPPET.load(Ordering::Relaxed) as usize;
    let next = (curr + 1) % 8;
    let name1 = get_snippet_name(curr);
    let text1 = get_snippet_text(curr);
    let name2 = get_snippet_name(next);
    let text2 = get_snippet_text(next);

    // Row 1: Selected Snippet (Inverted background)
    Rectangle::new(Point::new(0, 11), Size::new(127, 10))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)
        .ok();
    let mut s1: heapless::String<32> = heapless::String::new();
    let prev1 = text1.trim();
    let _ = core::fmt::write(&mut s1, format_args!(">{}.{}: {:.16}", curr + 1, name1, prev1));
    Text::new(&s1, Point::new(2, 18), MonoTextStyle::new(&FONT_4X6, BinaryColor::Off))
        .draw(display)
        .ok();

    // Row 2: Next Snippet (Outlined)
    Rectangle::new(Point::new(0, 22), Size::new(127, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    let mut s2: heapless::String<32> = heapless::String::new();
    let prev2 = text2.trim();
    let _ = core::fmt::write(&mut s2, format_args!(" {}.{}: {:.16}", next + 1, name2, prev2));
    Text::new(&s2, Point::new(2, 29), text_style)
        .draw(display)
        .ok();
}

fn draw_layout_preview<D: DrawTarget<Color = BinaryColor>>(layer: u8, display: &mut D) {
    if layer == 4 {
        draw_snippets_browser(display);
        return;
    }

    let layout = &LAYOUTS[(layer as usize) % 5];
    let box_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);

    // Row 0:
    // Screen title box: x = 0, y = 0, width = 62, height = 10
    Rectangle::new(Point::new(0, 0), Size::new(62, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    let mut title_buf: heapless::String<16> = heapless::String::new();
    let _ = core::fmt::write(&mut title_buf, format_args!("LYR: {}", layout.title));
    let tx = (62 - (title_buf.len() as i32 * 4)) / 2;
    Text::new(&title_buf, Point::new(tx, 7), text_style).draw(display).ok();

    // Knob box: x = 64, y = 0, width = 31, height = 10
    Rectangle::new(Point::new(64, 0), Size::new(31, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    let kx = 64 + (31 - (layout.knob.len() as i32 * 4)) / 2;
    Text::new(layout.knob, Point::new(kx, 7), text_style).draw(display).ok();

    // Top-right key: x = 97, y = 0, width = 31, height = 10
    Rectangle::new(Point::new(97, 0), Size::new(31, 10))
        .into_styled(box_style)
        .draw(display)
        .ok();
    let top_x = 97 + (31 - (layout.top_key.len() as i32 * 4)) / 2;
    Text::new(layout.top_key, Point::new(top_x, 7), text_style).draw(display).ok();

    // Row 1 & Row 2:
    if layer == 3 {
        // Full-width IP display box
        Rectangle::new(Point::new(0, 11), Size::new(127, 10))
            .into_styled(box_style)
            .draw(display)
            .ok();
        let ip_val = ASSIGNED_IP.load(Ordering::Relaxed);
        let mut ip_buf: heapless::String<24> = heapless::String::new();
        if ip_val != 0 {
            let b = ip_val.to_be_bytes();
            let _ = core::fmt::write(&mut ip_buf, format_args!("IP: {}.{}.{}.{}", b[0], b[1], b[2], b[3]));
        } else {
            let _ = ip_buf.push_str("IP: ACQUIRING...");
        }
        let lx = (127 - (ip_buf.len() as i32 * 4)) / 2;
        Text::new(&ip_buf, Point::new(lx, 18), text_style).draw(display).ok();

        // Row 2: Status (left 95px) and BOOT key (right 31px)
        Rectangle::new(Point::new(0, 22), Size::new(95, 10))
            .into_styled(box_style)
            .draw(display)
            .ok();
        let inet = INTERNET_STATUS.load(Ordering::Relaxed);
        let status_str = match inet {
            2 => "PORTAL:80 (NET OK)",
            3 => "PORTAL:80 (LOCAL)",
            _ => "PORTAL:80 (ACTIVE)",
        };
        let sx = (95 - (status_str.len() as i32 * 4)) / 2;
        Text::new(status_str, Point::new(sx, 29), text_style).draw(display).ok();

        Rectangle::new(Point::new(96, 22), Size::new(31, 10))
            .into_styled(box_style)
            .draw(display)
            .ok();
        Text::new("BOOT", Point::new(103, 29), text_style).draw(display).ok();
        return;
    }

    // Row 1: y = 11, height = 10 (4 keys)
    for (i, label) in layout.r1.iter().enumerate() {
        let x = (i as i32) * 32;
        Rectangle::new(Point::new(x, 11), Size::new(31, 10))
            .into_styled(box_style)
            .draw(display)
            .ok();
        let lx = x + (31 - (label.len() as i32 * 4)) / 2;
        Text::new(label, Point::new(lx, 18), text_style).draw(display).ok();
    }

    // Row 2: y = 22, height = 10 (4 keys)
    for (i, label) in layout.r2.iter().enumerate() {
        let x = (i as i32) * 32;
        Rectangle::new(Point::new(x, 22), Size::new(31, 10))
            .into_styled(box_style)
            .draw(display)
            .ok();
        let lx = x + (31 - (label.len() as i32 * 4)) / 2;
        Text::new(label, Point::new(lx, 29), text_style).draw(display).ok();
    }
}

fn draw_costume<D: DrawTarget<Color = BinaryColor>>(layer: u8, display: &mut D) {
    use embedded_graphics::primitives::Line;

    let stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let fill = PrimitiveStyle::with_fill(BinaryColor::On);

    match layer {
        1 => {
            // GAME: Gamer Headset
            Line::new(Point::new(52, 7), Point::new(60, 4)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(60, 4), Point::new(70, 3)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(70, 3), Point::new(78, 4)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(52, 8), Point::new(60, 5)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(60, 5), Point::new(70, 4)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(50, 8), Size::new(3, 6)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(77, 3), Size::new(3, 5)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(51, 14), Point::new(56, 16)).into_styled(stroke).draw(display).ok();
        }
        2 => {
            // MEDIA: Floating Music Notes
            Line::new(Point::new(94, 2), Point::new(94, 7)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(94, 2), Point::new(96, 4)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(91, 6), Size::new(4, 2)).into_styled(fill).draw(display).ok();

            Line::new(Point::new(107, 1), Point::new(115, 1)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(107, 2), Point::new(115, 2)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(107, 1), Point::new(107, 7)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(115, 1), Point::new(115, 6)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(104, 6), Size::new(4, 2)).into_styled(fill).draw(display).ok();
            Rectangle::new(Point::new(112, 5), Size::new(4, 2)).into_styled(fill).draw(display).ok();
        }
        3 => {
            // TOOLS: Wizard Hat
            Line::new(Point::new(67, 0), Point::new(60, 7)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(67, 0), Point::new(72, 7)).into_styled(stroke).draw(display).ok();
            Pixel(Point::new(66, 3), BinaryColor::On).draw(display).ok();
            Pixel(Point::new(67, 4), BinaryColor::On).draw(display).ok();
            Line::new(Point::new(58, 7), Point::new(75, 7)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(59, 8), Point::new(74, 8)).into_styled(stroke).draw(display).ok();
        }
        4 => {
            // SNIP: Glasses on Cat
            Line::new(Point::new(54, 11), Point::new(76, 11)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(53, 10), Size::new(8, 5)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(67, 10), Size::new(8, 5)).into_styled(stroke).draw(display).ok();
        }
        _ => {}
    }
}

impl DisplayRenderer<BinaryColor> for SniqxRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if IS_BOOTING.load(Ordering::Relaxed) != 0 {
            draw_rmk_logo(display);
            return;
        }
        if ctx.sleeping {
            return;
        }

        // --- Alert Popup Banner (Highest priority visual feedback) ---
        let alert = ALERT_TICKS.load(Ordering::Relaxed);
        if alert > 0 {
            ALERT_TICKS.store(alert - 1, Ordering::Relaxed);
            Rectangle::new(Point::new(2, 2), Size::new(124, 28))
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)
                .ok();
            Rectangle::new(Point::new(4, 4), Size::new(120, 24))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
                .draw(display)
                .ok();
            ALERT_BUF.lock(|cell| {
                let msg = cell.borrow();
                let x = (128 - (msg.len() as i32 * 6)) / 2;
                Text::new(&msg, Point::new(x.max(6), 19), MonoTextStyle::new(&FONT_6X10, BinaryColor::On))
                    .draw(display)
                    .ok();
            });
            return;
        }

        // --- 1. Track Base Layer & Layer Peek Logic ---
        if ctx.layer < 5 && ctx.layer != self.last_base_layer {
            self.last_base_layer = ctx.layer;
            self.preview_ticks = 20; // Auto-peek layout for 2.0s on layer switch
        }

        // Process Encoder Action (Clockwise = +, CounterClockwise = -)
        let action = ENCODER_ACTION.load(Ordering::Relaxed);
        if action != 0 {
            ENCODER_ACTION.store(0, Ordering::Relaxed);
            let ind = match (self.last_base_layer, action) {
                (2, 1) => "V+",
                (2, 2) => "V-",
                (3, 1) => "B+",
                (3, 2) => "B-",
                (4, 3) => "SNP",
                (_, 1) => "S+",
                (_, 2) => "S-",
                _ => "",
            };
            if !ind.is_empty() {
                self.knob_indicator = Some(ind);
                self.knob_indicator_ticks = 15;
            }
        } else if self.knob_indicator_ticks > 0 {
            self.knob_indicator_ticks -= 1;
            if self.knob_indicator_ticks == 0 {
                self.knob_indicator = None;
            }
        }

        // Holding key next to knob (Layer 5 via MO/LT) or Ctrl + Shift combo
        let is_holding_peek = ctx.layer == 5;
        let combo_preview = (ctx.modifiers.left_ctrl() || ctx.modifiers.right_ctrl())
            && (ctx.modifiers.left_shift() || ctx.modifiers.right_shift());

        if is_holding_peek || combo_preview {
            draw_layout_preview(self.last_base_layer, display);
            return;
        }

        // Auto-peek timeout on layer switch
        if self.preview_ticks > 0 {
            self.preview_ticks -= 1;
            draw_layout_preview(self.last_base_layer, display);
            return;
        }

        // On Layer 4 (SNIP), render the dedicated scrollable snippets browser
        if self.last_base_layer == 4 {
            draw_snippets_browser(display);
            return;
        }

        // --- 2. Update Bongo Cat Animation State ---
        if ctx.key_press_latch {
            self.tap_frame = 1 - self.tap_frame;
            self.tap_hold_ticks = 2;
            self.idle_ticks = 0;
        } else if ctx.key_pressed {
            self.tap_hold_ticks = 2;
            self.idle_ticks = 0;
        } else {
            if self.tap_hold_ticks > 0 {
                self.tap_hold_ticks -= 1;
            }
            self.idle_ticks = self.idle_ticks.wrapping_add(1);
        }

        let is_sleeping = self.idle_ticks > 400; // >40s of inactivity

        // Select frame
        let frame = if self.tap_hold_ticks > 0 {
            if self.tap_frame == 0 {
                &BONGO_TAP0
            } else {
                &BONGO_TAP1
            }
        } else if is_sleeping {
            &BONGO_BLINK // Closed eyes while sleeping
        } else {
            // Idle: occasional gentle blink
            let cycle = self.idle_ticks % 35;
            if cycle == 30 || cycle == 31 {
                &BONGO_BLINK
            } else {
                &BONGO_IDLE
            }
        };

        // --- 3. Draw Bongo Cat (128x32) ---
        let raw_cat = ImageRaw::<BinaryColor>::new(frame, 128);
        Image::new(&raw_cat, Point::zero()).draw(display).ok();

        // Draw Layer Costume Accessories on Cat
        if !is_sleeping {
            draw_costume(self.last_base_layer, display);
        } else {
            let z_step = (self.idle_ticks / 5) % 3;
            let z_font = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
            Text::new("z", Point::new(82, 14), z_font).draw(display).ok();
            if z_step >= 1 {
                Text::new("Z", Point::new(91, 9), z_font).draw(display).ok();
            }
            if z_step >= 2 {
                Text::new("Z", Point::new(100, 4), z_font).draw(display).ok();
            }
        }

        // --- 4. Draw Layer Badge on Left (x = 2..46, y = 2..18) ---
        let box_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        let layer_style = MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);

        let (layer_name, text_x) = match self.last_base_layer {
            0 => ("BASE", 11),
            1 => ("GAME", 11),
            2 => ("MEDIA", 7),
            3 => ("TOOLS", 7),
            4 => ("SNIP", 11),
            _ => ("EXT", 14),
        };

        Rectangle::new(Point::new(2, 2), Size::new(45, 17))
            .into_styled(box_style)
            .draw(display)
            .ok();
        Text::new(layer_name, Point::new(text_x, 14), layer_style)
            .draw(display)
            .ok();

        // --- 5. Draw Active Modifiers, Knob Indicator, or Layer Dots (y = 20..31) ---
        let mut mod_buf: heapless::String<10> = heapless::String::new();
        if ctx.modifiers.left_ctrl() || ctx.modifiers.right_ctrl() {
            let _ = mod_buf.push_str("CTL ");
        }
        if ctx.modifiers.left_shift() || ctx.modifiers.right_shift() {
            let _ = mod_buf.push_str("SFT ");
        }
        if ctx.modifiers.left_alt() || ctx.modifiers.right_alt() {
            let _ = mod_buf.push_str("ALT ");
        }
        if ctx.modifiers.left_gui() || ctx.modifiers.right_gui() {
            let _ = mod_buf.push_str("GUI ");
        }
        if ctx.caps_lock {
            let _ = mod_buf.push_str("CAPS");
        }

        if !mod_buf.is_empty() {
            let mod_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
            Text::new(&mod_buf, Point::new(4, 28), mod_style)
                .draw(display)
                .ok();
        } else if let Some(ind) = self.knob_indicator {
            Rectangle::new(Point::new(2, 20), Size::new(45, 11))
                .into_styled(box_style)
                .draw(display)
                .ok();
            let ind_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
            Text::new(ind, Point::new(18, 28), ind_style)
                .draw(display)
                .ok();
        } else if self.last_base_layer == 4 {
            Rectangle::new(Point::new(2, 20), Size::new(45, 11))
                .into_styled(box_style)
                .draw(display)
                .ok();
            let snip_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
            let curr = SELECTED_SNIPPET.load(Ordering::Relaxed) as usize;
            let name = get_snippet_name(curr);
            let mut snip_lbl: heapless::String<10> = heapless::String::new();
            let _ = core::fmt::write(&mut snip_lbl, format_args!(">{:.7}", name));
            Text::new(&snip_lbl, Point::new(4, 28), snip_style).draw(display).ok();
        } else if self.last_base_layer == 3 && ASSIGNED_IP.load(Ordering::Relaxed) != 0 {
            Rectangle::new(Point::new(2, 20), Size::new(45, 11))
                .into_styled(box_style)
                .draw(display)
                .ok();
            let net_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
            let inet = INTERNET_STATUS.load(Ordering::Relaxed);
            let lbl = if inet == 2 { "WWW:OK" } else { "NET:ON" };
            Text::new(lbl, Point::new(7, 28), net_style).draw(display).ok();
        } else {
            let dot_fill = PrimitiveStyle::with_fill(BinaryColor::On);
            let dot_stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
            let dot_xs = [5, 13, 21, 29, 37];

            for (i, &cx) in dot_xs.iter().enumerate() {
                let dot_origin = Point::new(cx, 22);
                if i as u8 == self.last_base_layer {
                    Circle::new(dot_origin, 4)
                        .into_styled(dot_fill)
                        .draw(display)
                        .ok();
                } else {
                    Circle::new(dot_origin, 4)
                        .into_styled(dot_stroke)
                        .draw(display)
                        .ok();
                }
            }
        }
    }
}
