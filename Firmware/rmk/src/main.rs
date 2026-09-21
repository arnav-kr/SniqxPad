#![no_main]
#![no_std]

mod bongo;

use bongo::{draw_rmk_logo, BONGO_BLINK, BONGO_IDLE, BONGO_TAP0, BONGO_TAP1};
use core::sync::atomic::{AtomicU8, Ordering};
use embedded_graphics::{
    image::{Image, ImageRaw},
    mono_font::{
        ascii::{FONT_5X8, FONT_6X10, FONT_7X13_BOLD},
        MonoTextStyle,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle},
    text::Text,
};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::event::{KeyboardEvent, KeyboardEventPos, LayerChangeEvent};
use rmk::input_device::rotary_encoder::Direction;
use rmk::macros::{processor, rmk_keyboard};

static ENCODER_ACTION: AtomicU8 = AtomicU8::new(0);
static CURRENT_LAYER: AtomicU8 = AtomicU8::new(0);
static IS_BOOTING: AtomicU8 = AtomicU8::new(0);

#[processor(subscribe = [KeyboardEvent, LayerChangeEvent])]
pub struct EncoderWatcher;

impl EncoderWatcher {
    async fn on_keyboard_event(&mut self, event: KeyboardEvent) {
        if let KeyboardEventPos::RotaryEncoder(rotary) = event.pos {
            if event.pressed {
                match rotary.direction {
                    Direction::Clockwise => ENCODER_ACTION.store(1, Ordering::Relaxed),
                    Direction::CounterClockwise => ENCODER_ACTION.store(2, Ordering::Relaxed),
                    Direction::None => {}
                }
            }
        } else if let KeyboardEventPos::Key(pos) = event.pos {
            if event.pressed && pos.row == 2 && pos.col == 3 {
                if CURRENT_LAYER.load(Ordering::Relaxed) == 3 {
                    IS_BOOTING.store(1, Ordering::Relaxed);
                }
            }
        }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        CURRENT_LAYER.store(event.0, Ordering::Relaxed);
    }
}

pub struct SniqxRenderer {
    tap_frame: usize,
    tap_hold_ticks: u8,
    idle_ticks: u16,
    last_base_layer: u8,
    preview_ticks: u8,
    knob_indicator: Option<&'static str>,
    knob_indicator_ticks: u8,
}

impl Default for SniqxRenderer {
    fn default() -> Self {
        Self {
            tap_frame: 0,
            tap_hold_ticks: 0,
            idle_ticks: 0,
            last_base_layer: 0,
            preview_ticks: 0,
            knob_indicator: None,
            knob_indicator_ticks: 0,
        }
    }
}

struct LayerLayout {
    title: &'static str,
    knob: &'static str,
    top_key: &'static str,
    r1: [&'static str; 4],
    r2: [&'static str; 4],
}

const LAYOUTS: [LayerLayout; 4] = [
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
];

fn draw_layout_preview<D: DrawTarget<Color = BinaryColor>>(layer: u8, display: &mut D) {
    use embedded_graphics::mono_font::ascii::FONT_4X6;

    let layout = &LAYOUTS[(layer as usize) % 4];
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
            // Headband arc from left ear to right ear
            Line::new(Point::new(52, 7), Point::new(60, 4)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(60, 4), Point::new(70, 3)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(70, 3), Point::new(78, 4)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(52, 8), Point::new(60, 5)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(60, 5), Point::new(70, 4)).into_styled(stroke).draw(display).ok();
            // Left earcup
            Rectangle::new(Point::new(50, 8), Size::new(3, 6)).into_styled(stroke).draw(display).ok();
            // Right earcup
            Rectangle::new(Point::new(77, 3), Size::new(3, 5)).into_styled(stroke).draw(display).ok();
            // Mic boom
            Line::new(Point::new(51, 14), Point::new(56, 16)).into_styled(stroke).draw(display).ok();
        }
        2 => {
            // MEDIA: Floating Music Notes
            // Single note (x = 94, y = 2)
            Line::new(Point::new(94, 2), Point::new(94, 7)).into_styled(stroke).draw(display).ok();
            Line::new(Point::new(94, 2), Point::new(96, 4)).into_styled(stroke).draw(display).ok();
            Rectangle::new(Point::new(91, 6), Size::new(4, 2)).into_styled(fill).draw(display).ok();

            // Beamed double note (x = 105, y = 1)
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

        // --- 1. Track Base Layer & Layer Peek Logic ---
        if ctx.layer < 4 {
            if ctx.layer != self.last_base_layer {
                self.last_base_layer = ctx.layer;
                self.preview_ticks = 20; // Auto-peek layout for 2.0s on layer switch
            }
        }

        // Process Encoder Action (Clockwise = +, CounterClockwise = -)
        let action = ENCODER_ACTION.load(Ordering::Relaxed);
        if action != 0 {
            ENCODER_ACTION.store(0, Ordering::Relaxed);
            let ind = match (self.last_base_layer, action) {
                // Layer 2: MEDIA -> Volume
                (2, 1) => "V+",
                (2, 2) => "V-",
                // Layer 3: TOOLS -> Brightness
                (3, 1) => "B+",
                (3, 2) => "B-",
                // Layer 0 / 1: BASE / GAME -> Scroll
                (_, 1) => "S+",
                (_, 2) => "S-",
                _ => "",
            };
            if !ind.is_empty() {
                self.knob_indicator = Some(ind);
                self.knob_indicator_ticks = 15; // Show for 1.5 seconds
            }
        } else if self.knob_indicator_ticks > 0 {
            self.knob_indicator_ticks -= 1;
            if self.knob_indicator_ticks == 0 {
                self.knob_indicator = None;
            }
        }

        // Holding key next to knob (Layer 4 via MO/LT) or Ctrl + Shift combo
        let is_holding_peek = ctx.layer == 4;
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
            // Animated "z Z Z" floating bubbles
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
        } else {
            let dot_fill = PrimitiveStyle::with_fill(BinaryColor::On);
            let dot_stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
            let dot_xs = [7, 17, 27, 37];

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

#[rmk_keyboard]
mod keyboard {
    #[register_processor(event)]
    fn encoder_watcher() -> crate::EncoderWatcher {
        crate::EncoderWatcher
    }
}
