use crate::colors::{ButtonColorScheme, ColorScheme};
use widgetry::{ButtonBuilder, ControlState, ScreenDims};

pub trait Buttons<'a> {
    fn btn_primary_dark(&self) -> ButtonBuilder<'a>;
    fn btn_primary_dark_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_dark().label_text(text)
    }
    fn btn_primary_dark_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_primary_dark().image_path(image_path))
    }
    fn btn_primary_dark_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_dark()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_secondary_dark(&self) -> ButtonBuilder<'a>;
    fn btn_secondary_dark_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_dark().label_text(text)
    }
    fn btn_secondary_dark_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_secondary_dark().image_path(image_path))
    }
    fn btn_secondary_dark_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_secondary_dark()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_primary_light(&self) -> ButtonBuilder<'a>;
    fn btn_primary_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_light().label_text(text)
    }
    fn btn_primary_light_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_primary_light().image_path(image_path))
    }
    fn btn_primary_light_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_light()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_secondary_light(&self) -> ButtonBuilder<'a>;
    fn btn_secondary_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_light().label_text(text)
    }
    fn btn_secondary_light_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_secondary_light().image_path(image_path))
    }
    fn btn_secondary_light_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_secondary_light()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_plain_dark(&self) -> ButtonBuilder<'a>;
    fn btn_plain_dark_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_dark().label_text(text)
    }
    fn btn_plain_dark_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain_dark().image_path(image_path))
    }

    fn btn_plain_light(&self) -> ButtonBuilder<'a>;
    fn btn_plain_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_light().label_text(text)
    }
    fn btn_plain_light_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain_light().image_path(image_path))
    }

    // Specific UI Elements

    /// title: name of previous screen, which you'll return to
    fn btn_back_light(&self, title: &'a str) -> ButtonBuilder<'a> {
        back_button(self.btn_plain_light(), title)
    }

    /// title: name of previous screen, which you'll return to
    fn btn_back_dark(&self, title: &'a str) -> ButtonBuilder<'a> {
        back_button(self.btn_plain_dark(), title)
    }

    fn btn_popup_light(&self, text: &'a str) -> ButtonBuilder<'a> {
        popup_button(self.btn_secondary_light(), text)
    }

    fn btn_popup_dark(&self, text: &'a str) -> ButtonBuilder<'a> {
        popup_button(self.btn_secondary_dark(), text)
    }

    fn btn_hotkey_light(&self, label: &str, key: Key) -> ButtonBuilder<'a>;

    fn btn_close(&self) -> ButtonBuilder<'a> {
        self.btn_plain_light_icon("system/assets/tools/close.svg")
    }
}

use widgetry::{Key, Line, Text};
impl<'a> Buttons<'a> for ColorScheme {
    fn btn_hotkey_light(&self, label: &str, key: Key) -> ButtonBuilder<'a> {
        let default = {
            let mut txt = Text::new();
            let key_txt = Line(key.describe()).fg(self.gui_style.hotkey_color);
            txt.append(key_txt);
            let label_text = Line(format!(" - {}", label)).fg(self.btn_primary_light.fg);
            txt.append(label_text);
            txt
        };

        let disabled = {
            let mut txt = Text::new();
            let key_txt = Line(key.describe()).fg(self.gui_style.hotkey_color.alpha(0.3));
            txt.append(key_txt);
            let label_text = Line(format!(" - {}", label)).fg(self.btn_primary_light.fg_disabled);
            txt.append(label_text);
            txt
        };

        self.btn_primary_light()
            .label_styled_text(default, ControlState::Default)
            .label_styled_text(disabled, ControlState::Disabled)
            .hotkey(key)
    }

    fn btn_primary_dark(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_primary_dark;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }

    fn btn_secondary_dark(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_dark;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }

    fn btn_plain_dark(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_dark;
        plain_builder(colors)
    }

    fn btn_primary_light(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_primary_light;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }

    fn btn_secondary_light(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_light;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }

    fn btn_plain_light(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_light;
        plain_builder(colors)
    }
}

fn plain_builder<'a>(color_scheme: &ButtonColorScheme) -> ButtonBuilder<'a> {
    ButtonBuilder::new()
        .label_color(color_scheme.fg, ControlState::Default)
        .label_color(color_scheme.fg_disabled, ControlState::Disabled)
        .image_color(color_scheme.fg, ControlState::Default)
        .image_color(color_scheme.fg_disabled, ControlState::Disabled)
        .bg_color(color_scheme.bg, ControlState::Default)
        .bg_color(color_scheme.bg_hover, ControlState::Hover)
        .bg_color(color_scheme.bg_disabled, ControlState::Disabled)
}

// Captures some constants for uniform styling of icon-only buttons
fn icon_button<'a>(builder: ButtonBuilder<'a>) -> ButtonBuilder<'a> {
    builder.padding(8.0).image_dims(20.0)
}

// TODO: Move this into impl ButtonBuilder?
fn back_button<'a>(builder: ButtonBuilder<'a>, title: &'a str) -> ButtonBuilder<'a> {
    // DESIGN REVIEW: this button seems absurdly large
    builder
        .image_path("system/assets/pregame/back.svg")
        .label_text(title)
        .padding_left(8.0)
        .font_size(30)
}

// TODO: inline this now that it's so simple
fn popup_button<'a>(builder: ButtonBuilder<'a>, title: &'a str) -> ButtonBuilder<'a> {
    builder.dropdown().label_text(title)
}
