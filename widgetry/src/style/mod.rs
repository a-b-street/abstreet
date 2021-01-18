use crate::{Color, Text};

pub mod buttons;

#[derive(Clone)]
pub struct Style {
    pub outline_thickness: f64,
    pub outline_color: Color,
    pub panel_bg: Color,
    pub hotkey_color: Color,
    pub hovering_color: Color,
    pub loading_tips: Text,
    pub btn_primary_dark: ButtonStyle,
    pub btn_secondary_dark: ButtonStyle,
    pub btn_primary_light: ButtonStyle,
    pub btn_secondary_light: ButtonStyle,
}

#[derive(Clone)]
pub struct ButtonStyle {
    pub fg: Color,
    pub fg_disabled: Color,
    pub outline: Color,
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_disabled: Color,
}

impl Style {
    pub fn standard() -> Style {
        Style {
            outline_thickness: 2.0,
            outline_color: Color::WHITE,
            panel_bg: Color::grey(0.4),
            hotkey_color: Color::GREEN,
            hovering_color: Color::ORANGE,
            loading_tips: Text::new(),

            // UI > Buttons
            btn_primary_dark: ButtonStyle {
                fg: hex("#4C4C4C"),
                fg_disabled: hex("#4C4C4C").alpha(0.3),
                bg: Color::WHITE.alpha(0.8),
                bg_hover: Color::WHITE,
                bg_disabled: Color::grey(0.6),
                outline: Color::WHITE.alpha(0.6),
            },
            btn_secondary_dark: ButtonStyle {
                fg: hex("#4C4C4C"),
                fg_disabled: hex("#4C4C4C").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#4C4C4C").alpha(0.1),
                bg_disabled: Color::grey(0.8),
                outline: hex("#4C4C4C"),
            },
            btn_primary_light: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#003046").alpha(0.8),
                bg_hover: hex("#003046"),
                bg_disabled: Color::grey(0.1),
                outline: hex("#003046").alpha(0.6),
            },
            btn_secondary_light: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#F2F2F2").alpha(0.1),
                bg_disabled: Color::grey(0.9),
                outline: hex("#F2F2F2"),
            },
        }
    }
}

// Convenience
fn hex(x: &str) -> Color {
    Color::hex(x)
}
