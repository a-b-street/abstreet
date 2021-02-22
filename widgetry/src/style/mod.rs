use crate::{Color, Text};

pub mod buttons;

#[derive(Clone)]
pub struct Style {
    pub outline_thickness: f64,
    pub outline_color: Color,
    pub panel_bg: Color,
    pub hotkey_color: Color,
    pub loading_tips: Text,
    pub btn_solid_panel: ButtonTheme,
    pub btn_outline_dark: ButtonTheme,
    pub btn_solid_floating: ButtonTheme,
    pub btn_solid_destructive: ButtonTheme,
    pub btn_outline_destructive: ButtonTheme,
    pub btn_solid: ButtonTheme,
    pub btn_outline: ButtonTheme,
}

#[derive(Clone)]
pub struct ButtonTheme {
    pub fg: Color,
    pub fg_disabled: Color,
    pub outline: Color,
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_disabled: Color,
}

impl ButtonTheme {
    pub fn btn_solid_panel() -> Self {
        ButtonTheme {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::WHITE.alpha(0.8),
            bg_hover: Color::WHITE,
            bg_disabled: Color::grey(0.6),
            outline: Color::WHITE.alpha(0.6),
        }
    }

    pub fn btn_outline_dark() -> Self {
        ButtonTheme {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#4C4C4C").alpha(0.1),
            bg_disabled: Color::grey(0.8),
            outline: hex("#4C4C4C"),
        }
    }

    pub fn btn_solid_floating() -> Self {
        ButtonTheme {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#003046").alpha(0.8),
            bg_hover: hex("#003046"),
            bg_disabled: Color::grey(0.1),
            outline: hex("#003046").alpha(0.6),
        }
    }

    pub fn btn_outline() -> Self {
        ButtonTheme {
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
            panel_bg: Color::grey(0.4),
            hotkey_color: Color::GREEN,
            loading_tips: Text::new(),

            // Buttons

            // TODO: light/dark are color scheme details that have leaked into Style
            // deprecate these and assign the specific colors we want in the color scheme builder
            btn_solid_panel: ButtonTheme::btn_solid_panel(),
            btn_outline_dark: ButtonTheme::btn_outline_dark(),
            btn_solid_floating: ButtonTheme::btn_solid_floating(),

            // legacy day theme
            btn_outline: ButtonTheme::btn_outline(),
            btn_solid: ButtonTheme::btn_solid_panel(),

            // TODO new day theme
            // btn_solid: ButtonTheme::btn_solid_floating(),
            // btn_outline: ButtonTheme::btn_outline_dark(),
            btn_solid_destructive: ButtonTheme {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E").alpha(0.6),
            },
            btn_outline_destructive: ButtonTheme {
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
