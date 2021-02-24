use crate::{Color, Text};

pub mod buttons;

#[derive(Clone)]
pub struct Style {
    pub outline_thickness: f64,
    pub outline_color: Color,
    pub panel_bg: Color,
    pub text_fg_color: Color,
    pub hotkey_color: Color,
    pub loading_tips: Text,
    pub btn_solid: ButtonStyle,
    pub btn_outline: ButtonStyle,
    pub btn_solid_floating: ButtonStyle,
    pub btn_solid_destructive: ButtonStyle,
    pub btn_outline_destructive: ButtonStyle,
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

impl ButtonStyle {
    pub fn btn_solid() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::WHITE.alpha(0.8),
            bg_hover: Color::WHITE,
            bg_disabled: Color::grey(0.6),
            outline: Color::WHITE.alpha(0.6),
        }
    }

    pub fn btn_outline_dark() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#4C4C4C").alpha(0.1),
            bg_disabled: Color::grey(0.8),
            outline: hex("#4C4C4C"),
        }
    }

    pub fn btn_solid_floating() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#003046").alpha(0.8),
            bg_hover: hex("#003046"),
            bg_disabled: Color::grey(0.1),
            outline: hex("#003046").alpha(0.6),
        }
    }

    pub fn btn_outline() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#F2F2F2").alpha(0.1),
            bg_disabled: Color::grey(0.5),
            outline: hex("#F2F2F2"),
        }
    }
}

impl Style {
    pub fn standard() -> Style {
        Style {
            outline_thickness: 2.0,
            outline_color: Color::WHITE,
            hotkey_color: Color::GREEN,
            loading_tips: Text::new(),

            // Buttons
            btn_solid: ButtonStyle::btn_solid(),

            // legacy day theme
            panel_bg: Color::grey(0.4),
            text_fg_color: Color::WHITE,
            btn_outline: ButtonStyle::btn_outline(),
            btn_solid_floating: ButtonStyle::btn_solid_floating(),
            // TODO new day theme
            // panel_bg: Color::WHITE.alpha(0.8),
            // text_fg_color: hex("#4C4C4C"),
            // btn_outline: ButtonStyle::btn_outline_dark(),
            // btn_solid_floating: ButtonStyle::btn_solid(),
            btn_solid_destructive: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E").alpha(0.6),
            },
            btn_outline_destructive: ButtonStyle {
                fg: hex("#FF5E5E"),
                fg_disabled: hex("#FF5E5E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#FF5E5E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E"),
            },
        }
    }
}

// Convenience
fn hex(x: &str) -> Color {
    Color::hex(x)
}
