#![no_main]
#![no_std]
#![allow(clippy::duplicated_attributes)]

mod bongo;
pub mod display;
pub mod hid;
pub mod net;
pub mod snippets;

pub use display::{set_alert, SniqxRenderer};

use core::sync::atomic::{AtomicU8, Ordering};
use rmk::event::{KeyboardEvent, KeyboardEventPos, LayerChangeEvent};
use rmk::input_device::rotary_encoder::Direction;
use rmk::macros::{processor, rmk_keyboard};

use crate::display::{ENCODER_ACTION, IS_BOOTING};
use crate::snippets::{paste_all_snippets, type_snippet_index, SELECTED_SNIPPET};

static CURRENT_LAYER: AtomicU8 = AtomicU8::new(0);

#[processor(subscribe = [KeyboardEvent, LayerChangeEvent])]
pub struct EncoderWatcher;

impl EncoderWatcher {
    async fn on_keyboard_event(&mut self, event: KeyboardEvent) {
        let layer = CURRENT_LAYER.load(Ordering::Relaxed);
        if let KeyboardEventPos::RotaryEncoder(rotary) = event.pos {
            if event.pressed {
                if layer == 4 {
                    match rotary.direction {
                        Direction::Clockwise => {
                            let curr = SELECTED_SNIPPET.load(Ordering::Relaxed);
                            let next = (curr + 1) % 8;
                            SELECTED_SNIPPET.store(next, Ordering::Relaxed);
                            ENCODER_ACTION.store(3, Ordering::Relaxed);
                        }
                        Direction::CounterClockwise => {
                            let curr = SELECTED_SNIPPET.load(Ordering::Relaxed);
                            let prev = if curr == 0 { 7 } else { curr - 1 };
                            SELECTED_SNIPPET.store(prev, Ordering::Relaxed);
                            ENCODER_ACTION.store(3, Ordering::Relaxed);
                        }
                        Direction::None => {}
                    }
                } else {
                    match rotary.direction {
                        Direction::Clockwise => ENCODER_ACTION.store(1, Ordering::Relaxed),
                        Direction::CounterClockwise => ENCODER_ACTION.store(2, Ordering::Relaxed),
                        Direction::None => {}
                    }
                }
            }
        } else if let KeyboardEventPos::Key(pos) = event.pos {
            if event.pressed {
                if layer == 3 && pos.row == 2 && pos.col == 3 {
                    IS_BOOTING.store(1, Ordering::Relaxed);
                } else if layer == 4 {
                    if pos.row == 0 && pos.col == 1 {
                        let sel = SELECTED_SNIPPET.load(Ordering::Relaxed) as usize;
                        type_snippet_index(sel).await;
                    } else if pos.row == 2 && pos.col == 3 {
                        set_alert("PASTE ALL", 30);
                        paste_all_snippets().await;
                    } else {
                        let target = match (pos.row, pos.col) {
                            (1, 0) => Some(0),
                            (1, 1) => Some(1),
                            (1, 2) => Some(2),
                            (1, 3) => Some(3),
                            (2, 0) => Some(4),
                            (2, 1) => Some(5),
                            (2, 2) => Some(6),
                            _ => None,
                        };
                        if let Some(idx) = target {
                            type_snippet_index(idx).await;
                        }
                    }
                }
            }
        }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        CURRENT_LAYER.store(event.0, Ordering::Relaxed);
    }
}

#[rmk_keyboard]
mod keyboard {
    #[register_processor(event)]
    fn encoder_watcher() -> crate::EncoderWatcher {
        crate::EncoderWatcher
    }

    #[Override(entry)]
    async fn custom_entry() {
        static NCM_STATE: ::static_cell::StaticCell<::embassy_usb::class::cdc_ncm::State<'static>> =
            ::static_cell::StaticCell::new();
        let ncm_state = NCM_STATE.init(::embassy_usb::class::cdc_ncm::State::new());

        let host_mac = [0x02, 0x00, 0x5E, 0xA0, 0x00, 0x01];
        let dev_mac = [0x02, 0x00, 0x5E, 0xA0, 0x00, 0x02];

        let mut usb_builder = ::rmk::usb::UsbTransport::builder(driver, rmk_config.device_config);

        let cdc_ncm = ::embassy_usb::class::cdc_ncm::CdcNcmClass::new(
            usb_builder.usb_builder(),
            ncm_state,
            host_mac,
            64,
        );

        let mut usb_transport = usb_builder.build().with_host_service(&host_service);

        static NET_STATE: ::static_cell::StaticCell<
            ::embassy_usb::class::cdc_ncm::embassy_net::State<1514, 4, 4>,
        > = ::static_cell::StaticCell::new();
        let net_state = NET_STATE.init(::embassy_usb::class::cdc_ncm::embassy_net::State::new());
        let (ncm_runner, net_device) = cdc_ncm.into_embassy_net_device(net_state, dev_mac);

        static STACK_RESOURCES: ::static_cell::StaticCell<::embassy_net::StackResources<6>> =
            ::static_cell::StaticCell::new();
        let stack_resources = STACK_RESOURCES.init(::embassy_net::StackResources::new());

        let static_cfg = crate::net::get_static_config();

        let seed = 0x1234_5678_9abc_def0;
        let (stack, mut stack_runner) = ::embassy_net::new(
            net_device,
            ::embassy_net::Config::ipv4_static(static_cfg),
            stack_resources,
            seed,
        );

        let mut wpm_processor = ::rmk::processor::builtin::wpm::WpmProcessor::new();

        use ::rmk::core_traits::Runnable as _;

        ::embassy_futures::join::join5(
            usb_transport.run(),
            ncm_runner.run(),
            async { stack_runner.run().await },
            crate::net::net_task(stack),
            ::embassy_futures::join::join(
                wpm_processor.run(),
                ::rmk::run_all!(matrix, storage, keyboard, display_processor, encoder_0, encoder_watcher, watchdog_runner),
            ),
        )
        .await;
    }
}
