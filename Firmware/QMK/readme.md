# Sniqx Pad V1

![sniqx_pad](https://github.com/user-attachments/assets/335b4143-3be4-466b-bec2-c9d216802052)

A Versatile Macro Pad with a rotary encoder and OLED display.

* Keyboard Maintainer: [Arnav Kumar](https://github.com/arnav-kr)
* Hardware Supported: *xiao-rp2040*

Make example for this keyboard (after setting up your build environment):
```
make sniqx_pad:via
````
Flashing example for this keyboard:
```
make sniqx_pad:via:flash
```
See the [build environment setup](https://docs.qmk.fm/#/getting_started_build_tools) and the [make instructions](https://docs.qmk.fm/#/getting_started_make_guide) for more information. Brand new to QMK? Start with our [Complete Newbs Guide](https://docs.qmk.fm/#/newbs).

## Bootloader

Enter the bootloader in 3 ways:

* **Bootmagic reset**: Hold down the Rotary Encoder and plug in the keyboard
* **Physical reset button**: Hold down the `boot` button on the Xiao and plug in the keyboard
* **Keycode in layout**: Press the key mapped to `QK_BOOT` if it is available
