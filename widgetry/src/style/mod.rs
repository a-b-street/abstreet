use crate::{Color, Text};

pub mod buttons;

#[derive(Clone)]
pub struct Style {
    pub outline_thickness: f64,
    pub outline_color: Color,
    pub panel_bg: Color,
    pub field_bg: Color,
    pub dropdown_border: Color,
    pub icon_fg: Color,
    pub text_fg_color: Color,
    pub text_tooltip_color: Color,
    pub text_hotkey_color: Color,
    pub text_destructive_color: Color,
    pub loading_tips: Text,
    pub btn_solid: ButtonStyle,
    pub btn_outline: ButtonStyle,
    pub btn_solid_floating: ButtonStyle,
    pub btn_solid_destructive: ButtonStyle,
    pub btn_outline_destructive: ButtonStyle,
    pub btn_solid_primary: ButtonStyle,
    pub btn_outline_primary: ButtonStyle,
}

#[derive(Clone)]
pub struct ButtonStyle {
    pub fg: Color,
    pub fg_disabled: Color,
    pub outline: Color,
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_disabled: Color,
    pub outline_thickness: f64,
}

impl ButtonStyle {
    pub fn solid_dark_fg() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::WHITE.alpha(0.8),
            bg_hover: Color::WHITE,
            bg_disabled: Color::grey(0.6),
            outline: Color::WHITE.alpha(0.6),
            outline_thickness: 2.0,
        }
    }

    pub fn outline_dark_fg() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#4C4C4C").alpha(0.1),
            bg_disabled: Color::grey(0.8),
            outline: hex("#4C4C4C"),
            outline_thickness: 2.0,
        }
    }

    pub fn solid_light_fg() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#003046").alpha(0.8),
            bg_hover: hex("#003046"),
            bg_disabled: Color::grey(0.1),
            outline: hex("#003046").alpha(0.6),
            outline_thickness: 2.0,
        }
    }

    pub fn outline_light_fg() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#F2F2F2").alpha(0.1),
            bg_disabled: Color::grey(0.5),
            outline: hex("#F2F2F2"),
            outline_thickness: 2.0,
        }
    }
}

impl Style {
    pub fn light_bg() -> Style {
        Style {
            panel_bg: Color::WHITE.alpha(0.8),
            field_bg: hex("#F2F2F2"),
            dropdown_border: hex("#4C4C4C"),
            outline_thickness: 2.0,
            outline_color: hex("#4C4C4C"),
            loading_tips: Text::new(),
            icon_fg: hex("#4C4C4C"),
            text_fg_color: hex("#4C4C4C"),
            text_hotkey_color: hex("#EE702E"),
            text_tooltip_color: Color::WHITE,
            text_destructive_color: hex("#FF5E5E"),
            btn_outline: ButtonStyle::outline_dark_fg(),
            btn_solid: ButtonStyle {
                fg: Color::WHITE,
                fg_disabled: Color::WHITE.alpha(0.3),
                bg: hex("#4C4C4C").alpha(0.8),
                bg_hover: hex("#4C4C4C"),
                bg_disabled: Color::grey(0.6),
                outline: hex("#4C4C4C").alpha(0.6),
                outline_thickness: 2.0,
            },
            btn_solid_floating: ButtonStyle::solid_dark_fg(),
            btn_solid_destructive: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E").alpha(0.6),
                outline_thickness: 2.0,
            },
            btn_outline_destructive: ButtonStyle {
                fg: hex("#FF5E5E"),
                fg_disabled: hex("#FF5E5E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#FF5E5E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E"),
                outline_thickness: 2.0,
            },
            btn_solid_primary: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#EE702E").alpha(0.8),
                bg_hover: hex("#EE702E"),
                bg_disabled: hex("#EE702E").alpha(0.3),
                outline: hex("#EE702E").alpha(0.6),
                outline_thickness: 2.0,
            },
            btn_outline_primary: ButtonStyle {
                fg: hex("#EE702E"),
                fg_disabled: hex("#EE702E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#EE702E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: hex("#EE702E"),
                outline_thickness: 2.0,
            },
        }
    }

    pub fn pregame() -> Style {
        Style {
            panel_bg: Color::grey(0.4),
            field_bg: Color::grey(0.3),
            dropdown_border: Color::WHITE,
            outline_thickness: 2.0,
            outline_color: Color::WHITE,
            loading_tips: Text::new(),
            icon_fg: Color::WHITE,
            text_fg_color: Color::WHITE,
            text_hotkey_color: Color::GREEN,
            text_tooltip_color: Color::WHITE,
            text_destructive_color: hex("#EB3223"),
            btn_outline: ButtonStyle::outline_light_fg(),
            btn_solid: ButtonStyle::solid_dark_fg(),
            btn_solid_floating: ButtonStyle::solid_light_fg(),
            btn_solid_destructive: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E").alpha(0.6),
                outline_thickness: 2.0,
            },
            btn_outline_destructive: ButtonStyle {
                fg: hex("#FF5E5E"),
                fg_disabled: hex("#FF5E5E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#FF5E5E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: hex("#FF5E5E"),
                outline_thickness: 2.0,
            },
            btn_solid_primary: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#EE702E").alpha(0.8),
                bg_hover: hex("#EE702E"),
                bg_disabled: hex("#EE702E").alpha(0.3),
                outline: hex("#EE702E").alpha(0.6),
                outline_thickness: 2.0,
            },
            btn_outline_primary: ButtonStyle {
                fg: hex("#EE702E"),
                fg_disabled: hex("#EE702E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#EE702E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: hex("#EE702E"),
                outline_thickness: 2.0,
            },
        }
    }

    pub fn dark_bg() -> Style {
        let mut style = Self::light_bg();
        style.outline_color = Color::WHITE;
        style.panel_bg = hex("#003046").alpha(0.9);
        style.field_bg = style.panel_bg.shade(0.2);
        style.btn_outline = ButtonStyle::outline_light_fg();
        style.btn_solid = ButtonStyle::solid_dark_fg();
        style.btn_solid_floating = ButtonStyle::solid_light_fg();
        style.text_fg_color = Color::WHITE;
        style.icon_fg = Color::WHITE;
        style.text_hotkey_color = Color::GREEN;
        style.text_destructive_color = hex("#FF5E5E");
        style.dropdown_border = Color::WHITE;
        style
    }
}

// Convenience
fn hex(x: &str) -> Color {
    Color::hex(x)
}
