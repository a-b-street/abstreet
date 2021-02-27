use crate::{Color, Text};

pub mod buttons;

#[derive(Clone)]
pub struct Style {
    pub panel_bg: Color,
    pub field_bg: Color,
    pub dropdown_border: Color,
    pub icon_fg: Color,
    pub text_fg_color: Color,
    pub text_tooltip_color: Color,
    pub text_hotkey_color: Color,
    pub text_destructive_color: Color,
    pub loading_tips: Text,
    pub section_bg: Color,
    pub section_outline: OutlineStyle,
    pub btn_tab: ButtonStyle,
    pub btn_outline: ButtonStyle,
    pub btn_floating: ButtonStyle,
    pub btn_solid_destructive: ButtonStyle,
    pub btn_outline_destructive: ButtonStyle,
    pub btn_solid_primary: ButtonStyle,
    pub btn_outline_primary: ButtonStyle,
}

pub type OutlineStyle = (f64, Color);

#[derive(Clone)]
pub struct ButtonStyle {
    pub fg: Color,
    pub fg_disabled: Color,
    pub outline: OutlineStyle,
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_disabled: Color,
}

static DEFAULT_OUTLINE_THICKNESS: f64 = 2.0;

impl ButtonStyle {
    pub fn solid_dark_fg() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::WHITE.alpha(0.8),
            bg_hover: Color::WHITE,
            bg_disabled: Color::grey(0.6),
            outline: (DEFAULT_OUTLINE_THICKNESS, Color::WHITE.alpha(0.6)),
        }
    }

    pub fn outline_dark_fg() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#4C4C4C").alpha(0.1),
            bg_disabled: Color::CLEAR,
            outline: (DEFAULT_OUTLINE_THICKNESS, hex("#4C4C4C")),
        }
    }

    pub fn solid_light_fg() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#003046").alpha(0.8),
            bg_hover: hex("#003046"),
            bg_disabled: Color::grey(0.1),
            outline: (DEFAULT_OUTLINE_THICKNESS, hex("#003046").alpha(0.6)),
        }
    }

    pub fn outline_light_fg() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#F2F2F2").alpha(0.1),
            bg_disabled: Color::CLEAR,
            outline: (DEFAULT_OUTLINE_THICKNESS, hex("#F2F2F2")),
        }
    }
}

impl Style {
    pub fn light_bg() -> Style {
        Style {
            panel_bg: Color::WHITE.alpha(0.8),
            field_bg: hex("#F2F2F2"),
            dropdown_border: hex("#4C4C4C"),
            // TODO: replace inner_panel_bg with this
            section_bg: Color::WHITE,
            section_outline: (2.0, Color::WHITE.shade(0.1)),
            loading_tips: Text::new(),
            icon_fg: hex("#4C4C4C"),
            text_fg_color: hex("#4C4C4C"),
            text_hotkey_color: hex("#EE702E"),
            text_tooltip_color: Color::WHITE,
            text_destructive_color: hex("#FF5E5E"),
            btn_outline: ButtonStyle::outline_dark_fg(),
            btn_tab: ButtonStyle {
                fg: Color::WHITE,
                fg_disabled: Color::WHITE.alpha(0.3),
                bg: hex("#4C4C4C").alpha(0.8),
                bg_hover: hex("#4C4C4C"),
                bg_disabled: Color::grey(0.6),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#4C4C4C").alpha(0.6)),
            },
            btn_floating: ButtonStyle::solid_dark_fg(),
            btn_solid_destructive: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#FF5E5E").alpha(0.6)),
            },
            btn_outline_destructive: ButtonStyle {
                fg: hex("#FF5E5E"),
                fg_disabled: hex("#FF5E5E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#FF5E5E").alpha(0.1),
                bg_disabled: Color::CLEAR,
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#FF5E5E")),
            },
            btn_solid_primary: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#EE702E").alpha(0.8),
                bg_hover: hex("#EE702E"),
                bg_disabled: hex("#EE702E").alpha(0.3),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#EE702E").alpha(0.6)),
            },
            btn_outline_primary: ButtonStyle {
                fg: hex("#EE702E"),
                fg_disabled: hex("#EE702E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#EE702E").alpha(0.1),
                bg_disabled: Color::CLEAR,
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#EE702E")),
            },
        }
    }

    pub fn pregame() -> Style {
        Style {
            panel_bg: Color::grey(0.4),
            field_bg: Color::grey(0.3),
            dropdown_border: Color::WHITE,
            section_bg: Color::grey(0.5),
            section_outline: (2.0, Color::WHITE),
            loading_tips: Text::new(),
            icon_fg: Color::WHITE,
            text_fg_color: Color::WHITE,
            text_hotkey_color: Color::GREEN,
            text_tooltip_color: Color::WHITE,
            text_destructive_color: hex("#EB3223"),
            btn_outline: ButtonStyle::outline_light_fg(),
            btn_tab: ButtonStyle::solid_dark_fg(),
            btn_floating: ButtonStyle::solid_light_fg(),
            btn_solid_destructive: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#FF5E5E").alpha(0.8),
                bg_hover: hex("#FF5E5E"),
                bg_disabled: Color::grey(0.1),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#FF5E5E").alpha(0.6)),
            },
            btn_outline_destructive: ButtonStyle {
                fg: hex("#FF5E5E"),
                fg_disabled: hex("#FF5E5E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#FF5E5E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#FF5E5E")),
            },
            btn_solid_primary: ButtonStyle {
                fg: hex("#F2F2F2"),
                fg_disabled: hex("#F2F2F2").alpha(0.3),
                bg: hex("#EE702E").alpha(0.8),
                bg_hover: hex("#EE702E"),
                bg_disabled: hex("#EE702E").alpha(0.3),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#EE702E").alpha(0.6)),
            },
            btn_outline_primary: ButtonStyle {
                fg: hex("#EE702E"),
                fg_disabled: hex("#EE702E").alpha(0.3),
                bg: Color::CLEAR,
                bg_hover: hex("#EE702E").alpha(0.1),
                bg_disabled: Color::grey(0.1),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#EE702E")),
            },
        }
    }

    pub fn dark_bg() -> Style {
        let navy = hex("#003046");
        let mut style = Self::light_bg();
        style.panel_bg = navy.alpha(0.9);
        style.section_outline.1 = navy.shade(0.2);
        style.section_bg = navy;
        style.field_bg = navy.shade(0.2);
        style.btn_outline = ButtonStyle::outline_light_fg();
        style.btn_tab = ButtonStyle::solid_dark_fg();
        style.btn_floating = ButtonStyle::solid_light_fg();
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
