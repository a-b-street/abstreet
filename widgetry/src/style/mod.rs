use crate::{Color, Text};

pub mod button_style;
pub use button_style::ButtonStyle;

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
    pub btn_plain: ButtonStyle,
    pub btn_outline: ButtonStyle,
    pub btn_floating: ButtonStyle,
    pub btn_tab: ButtonStyle,
    pub btn_solid_destructive: ButtonStyle,
    pub btn_plain_destructive: ButtonStyle,
    pub btn_solid_primary: ButtonStyle,
    pub btn_plain_primary: ButtonStyle,
}

pub type OutlineStyle = (f64, Color);

static DEFAULT_OUTLINE_THICKNESS: f64 = 2.0;

// Some ButtonStyles are shared across Styles
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

    pub fn plain_dark_fg() -> Self {
        ButtonStyle {
            fg: hex("#4C4C4C"),
            fg_disabled: hex("#4C4C4C").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#4C4C4C").alpha(0.1),
            bg_disabled: Color::CLEAR,
            outline: (0.0, Color::CLEAR),
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

    pub fn plain_light_fg() -> Self {
        ButtonStyle {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#F2F2F2").alpha(0.1),
            bg_disabled: Color::CLEAR,
            outline: (0.0, Color::CLEAR),
        }
    }

    pub fn solid_primary() -> Self {
        Self {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#EE702E").alpha(0.8),
            bg_hover: hex("#EE702E"),
            bg_disabled: hex("#EE702E").alpha(0.3),
            outline: (DEFAULT_OUTLINE_THICKNESS, hex("#EE702E").alpha(0.6)),
        }
    }

    pub fn plain_primary() -> Self {
        Self {
            fg: hex("#EE702E"),
            fg_disabled: hex("#EE702E").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#EE702E").alpha(0.1),
            bg_disabled: Color::CLEAR,
            outline: (0.0, Color::CLEAR),
        }
    }

    fn solid_destructive() -> Self {
        Self {
            fg: hex("#F2F2F2"),
            fg_disabled: hex("#F2F2F2").alpha(0.3),
            bg: hex("#FF5E5E").alpha(0.8),
            bg_hover: hex("#FF5E5E"),
            bg_disabled: Color::grey(0.1),
            outline: (DEFAULT_OUTLINE_THICKNESS, hex("#FF5E5E").alpha(0.6)),
        }
    }

    fn plain_destructive() -> ButtonStyle {
        Self {
            fg: hex("#FF5E5E"),
            fg_disabled: hex("#FF5E5E").alpha(0.3),
            bg: Color::CLEAR,
            bg_hover: hex("#FF5E5E").alpha(0.1),
            bg_disabled: Color::grey(0.1),
            outline: (0.0, Color::CLEAR),
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
            btn_plain: ButtonStyle::plain_dark_fg(),
            btn_tab: ButtonStyle {
                fg: Color::WHITE,
                fg_disabled: Color::WHITE.alpha(0.3),
                bg: hex("#4C4C4C").alpha(0.8),
                bg_hover: hex("#4C4C4C"),
                bg_disabled: Color::grey(0.6),
                outline: (DEFAULT_OUTLINE_THICKNESS, hex("#4C4C4C").alpha(0.6)),
            },
            btn_floating: ButtonStyle::solid_dark_fg(),
            btn_solid_destructive: ButtonStyle::solid_destructive(),
            btn_plain_destructive: ButtonStyle::plain_destructive(),
            btn_solid_primary: ButtonStyle::solid_primary(),
            btn_plain_primary: ButtonStyle::plain_primary(),
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
            btn_tab: ButtonStyle::solid_dark_fg(),
            btn_outline: ButtonStyle::outline_light_fg(),
            btn_plain: ButtonStyle::plain_light_fg(),
            btn_floating: ButtonStyle::solid_light_fg(),
            btn_solid_destructive: ButtonStyle::solid_destructive(),
            btn_plain_destructive: ButtonStyle::plain_destructive(),
            btn_solid_primary: ButtonStyle::solid_primary(),
            btn_plain_primary: ButtonStyle::plain_primary(),
        }
    }

    pub fn dark_bg() -> Style {
        let navy = hex("#003046");
        Style {
            panel_bg: navy.alpha(0.9),
            field_bg: navy.shade(0.2),
            dropdown_border: Color::WHITE,
            // TODO: replace inner_panel_bg with this
            section_bg: navy,
            section_outline: (DEFAULT_OUTLINE_THICKNESS, navy.shade(0.2)),
            loading_tips: Text::new(),
            icon_fg: Color::WHITE,
            text_fg_color: Color::WHITE,
            text_hotkey_color: Color::GREEN,
            text_tooltip_color: Color::WHITE,
            text_destructive_color: hex("#FF5E5E"),
            btn_outline: ButtonStyle::outline_light_fg(),
            btn_plain: ButtonStyle::plain_light_fg(),
            btn_tab: ButtonStyle::solid_dark_fg(),
            btn_floating: ButtonStyle::solid_light_fg(),
            btn_solid_destructive: ButtonStyle::solid_destructive(),
            btn_plain_destructive: ButtonStyle::plain_destructive(),
            btn_solid_primary: ButtonStyle::solid_primary(),
            btn_plain_primary: ButtonStyle::plain_primary(),
        }
    }
}

// Convenience
fn hex(x: &str) -> Color {
    Color::hex(x)
}
