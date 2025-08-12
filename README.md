# Sniqx Pad V1


A Versatile Macro Pad with a rotary encoder and OLED display.

<table>
  <tr>
    <td>
      <img src="images/sniqx_pad.png" alt="Sniqx Pad" width="500"/>
    </td>
    <td>
      <img src="https://github.com/user-attachments/assets/335b4143-3be4-466b-bec2-c9d216802052" alt="Sniqx Pad" width="500"/>
    </td>
  </tr>
</table>


![Components](images/components.png)

## Specifications

### BOM
- 9x Cherry MX Switches
- 9x Blank DSA Keycaps (1x Red, 2x White, 6x Black)
- 1x Rotary Encoder
- 1x 128x32 OLED Display
- 10x 1N4148 Diodes
- 1x XIAO RP2040
- 4x M3x16 Bolt

## Usage
1. get the [sniqx_pad_via.uf2](https://github.com/arnav-kr/SniqxPad/releases/latest/download/sniqx_pad_via.uf2) and [via_config.json](https://github.com/arnav-kr/SniqxPad/releases/latest/download/via_config.json) from the [latest release](https://github.com/arnav-kr/SniqxPad/releases/latest).
2. Flash `sniqx_pad_via.uf2` to your xiao-rp2040 by dropping the uf2 into your xiao drive.
3. for configuration using via, use https://www.usevia.app/ (see below)

### Configuring usevia.app
1. Enable the Design Tab in settings 
    <img width="944" height="110" alt="image" src="https://github.com/user-attachments/assets/30d60ef3-6161-4fc3-9ed4-12debc30b789" />
    
2. In Design Tab upload the `via_config.json` that you downloaded from latest release
    <img width="942" height="590" alt="image" src="https://github.com/user-attachments/assets/8172695f-054e-4e75-bca9-e9d87cd56092" />

3. Authorize your device while it is connected via usb, and select the device in popup.
    <img width="934" height="845" alt="image" src="https://github.com/user-attachments/assets/cb1adef1-541d-4374-935c-7f4e8ea77cec" />

4. Profit! Configure your keyboard however you want.
    <img width="946" height="901" alt="image" src="https://github.com/user-attachments/assets/c2089bbb-a172-47d5-b372-6edc855a39ad" />


### Other Resources
- [PCB](PCB)
- [CAD Models](CAD)
- [Firmware](Firmware)
- [Images](images)

|             Schematic              |          PCB           |           Case           |
| :--------------------------------: | :--------------------: | :----------------------: |
| ![schematic](images/schematic.png) | ![pcb](images/pcb.png) | ![case](images/case.png) |

## Author
- [Arnav Kumar](https://github.com/arnav-kr)
