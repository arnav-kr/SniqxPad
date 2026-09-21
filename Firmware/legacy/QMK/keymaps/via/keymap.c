#include QMK_KEYBOARD_H

#define NUM_LAYERS 4

enum custom_keycodes
{
    LYR_CYC = SAFE_RANGE,
};

const uint16_t PROGMEM keymaps[4][MATRIX_ROWS][MATRIX_COLS] = {
    /* BASE Layer 0 */
    [0] = LAYOUT_ortho_3x4(
        LYR_CYC, KC_P,
        KC_ESC, KC_D, KC_I, KC_ENT,
        KC_H, KC_J, KC_K, KC_L),
    /* GAME Layer 1 */
    [1] = LAYOUT_ortho_3x4(
        KC_TRNS, KC_ESC,
        KC_LSFT, KC_Q, KC_W, KC_E,
        KC_LCTL, KC_A, KC_S, KC_D),
    /* MEDIA Layer 2 */
    [2] = LAYOUT_ortho_3x4(
        KC_TRNS, KC_TRNS,
        KC_MPLY, KC_MPRV, KC_MNXT, KC_VOLU,
        KC_MUTE, KC_TRNS, KC_TRNS, KC_VOLD),
    /* TOOLS Layer 3 */
    [3] = LAYOUT_ortho_3x4(
        KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS,
        KC_TRNS, KC_TRNS, KC_TRNS, KC_TRNS)};
const uint16_t PROGMEM encoder_map[4][1][2] = {
    [0] = {ENCODER_CCW_CW(MS_WHLD, MS_WHLU)},
    [1] = {ENCODER_CCW_CW(MS_WHLD, MS_WHLU)},
    [2] = {ENCODER_CCW_CW(KC_VOLU, KC_VOLD)},
    [3] = {ENCODER_CCW_CW(MS_WHLD, MS_WHLU)},
};

bool process_record_user(uint16_t keycode, keyrecord_t *record)
{
    if (record->event.pressed)
    {
        switch (keycode)
        {
        case LYR_CYC:
        {
            uint8_t current_layer = get_highest_layer(layer_state);
            uint8_t next_layer = (current_layer + 1) % NUM_LAYERS;
            layer_move(next_layer);
            return false;
        }
        }
    }
    return true;
}

oled_rotation_t oled_init_user(oled_rotation_t rotation)
{
    return OLED_ROTATION_180;
}

bool oled_task_user(void)
{
    uint8_t current_layer = get_highest_layer(layer_state);
    oled_set_cursor(0, 0);
    oled_write_P(PSTR("Layer: "), false);
    switch (current_layer)
    {
    case 0:
        oled_write_P(PSTR("BASE"), false);
        break;
    case 1:
        oled_write_P(PSTR("GAME"), false);
        break;
    case 2:
        oled_write_P(PSTR("MEDIA"), false);
        break;
    default:
        oled_write_P(PSTR("TOOLS"), false);
        break;
    }
    return true;
}
