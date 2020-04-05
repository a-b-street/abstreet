use crate::Color;

#[derive(Clone)]
pub struct Style {
    pub outline_thickness: f64,
    pub outline_color: Color,
    pub panel_bg: Color,
    pub hotkey_color: Color,
}

impl Style {
    pub fn standard() -> Style {
        Style {
            outline_thickness: 2.0,
            outline_color: Color::WHITE,
            panel_bg: Color::grey(0.4),
            hotkey_color: Color::GREEN,
        }
    }
}
