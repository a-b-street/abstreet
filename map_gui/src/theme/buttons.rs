use crate::ColorScheme;
use widgetry::{ButtonBuilder, ButtonState, Color};

pub trait Btn<'a> {
    fn btn_svg(&self, path: &'a str) -> ButtonBuilder<'a>;

    fn btn_primary_dark(&self) -> ButtonBuilder<'a>;
    fn btn_primary_dark_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_dark().label_text(text)
    }

    fn btn_secondary_dark(&self) -> ButtonBuilder<'a>;
    fn btn_secondary_dark_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_dark().label_text(text)
    }

    fn btn_primary_light(&self) -> ButtonBuilder<'a>;
    fn btn_primary_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_light().label_text(text)
    }

    fn btn_secondary_light(&self) -> ButtonBuilder<'a>;
    fn btn_secondary_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_light().label_text(text)
    }
}

impl<'a> Btn<'a> for ColorScheme {
    fn btn_svg(&self, path: &'a str) -> ButtonBuilder<'a> {
        ButtonBuilder::new()
            .image_path(&path)
            .bg_color(self.hovering, ButtonState::Hover)
    }

    fn btn_primary_dark(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_primary_dark;
        ButtonBuilder::new()
            .label_color(colors.fg, ButtonState::Default)
            .bg_color(colors.bg, ButtonState::Default)
            .bg_color(colors.bg_hover, ButtonState::Hover)
            .outline(2.0, colors.outline, ButtonState::Default)
    }

    fn btn_secondary_dark(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_dark;
        ButtonBuilder::new()
            .label_color(colors.fg, ButtonState::Default)
            .bg_color(colors.bg, ButtonState::Default)
            .bg_color(colors.bg_hover, ButtonState::Hover)
            .outline(2.0, colors.outline, ButtonState::Default)
    }

    fn btn_primary_light(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_primary_light;
        ButtonBuilder::new()
            .label_color(colors.fg, ButtonState::Default)
            .bg_color(colors.bg, ButtonState::Default)
            .bg_color(colors.bg_hover, ButtonState::Hover)
            .outline(2.0, colors.outline, ButtonState::Default)
    }

    fn btn_secondary_light(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_light;
        ButtonBuilder::new()
            .label_color(colors.fg, ButtonState::Default)
            .bg_color(colors.bg, ButtonState::Default)
            .bg_color(colors.bg_hover, ButtonState::Hover)
            .outline(2.0, colors.outline, ButtonState::Default)
    }
}
