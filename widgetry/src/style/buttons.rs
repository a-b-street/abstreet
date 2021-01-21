use super::ButtonStyle;
use crate::{ButtonBuilder, ControlState, EventCtx, ScreenDims, Style, Widget};

pub trait StyledButtons<'a> {
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
    fn btn_plain_dark_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_dark()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_plain_light(&self) -> ButtonBuilder<'a>;
    fn btn_plain_light_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_light().label_text(text)
    }
    fn btn_plain_light_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain_light().image_path(image_path))
    }
    fn btn_plain_light_icon_text(&self, image_path: &'a str, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_light()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_plain_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_plain_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_plain_destructive().label_text(text)
    }
    fn btn_plain_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_plain_destructive().image_path(image_path))
    }

    fn btn_primary_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_primary_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_primary_destructive().label_text(text)
    }
    fn btn_primary_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_primary_destructive().image_path(image_path))
    }
    fn btn_primary_destructive_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_primary_destructive()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
    }

    fn btn_secondary_destructive(&self) -> ButtonBuilder<'a>;
    fn btn_secondary_destructive_text(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_destructive().label_text(text)
    }
    fn btn_secondary_destructive_icon(&self, image_path: &'a str) -> ButtonBuilder<'a> {
        icon_button(self.btn_secondary_destructive().image_path(image_path))
    }
    fn btn_secondary_destructive_icon_text(
        &self,
        image_path: &'a str,
        text: &'a str,
    ) -> ButtonBuilder<'a> {
        self.btn_secondary_destructive()
            .label_text(text)
            .image_path(image_path)
            .image_dims(ScreenDims::square(18.0))
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

    fn btn_primary_light_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_primary_light())
    }

    fn btn_secondary_light_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_secondary_light())
    }

    fn btn_primary_dark_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_primary_dark())
    }

    fn btn_secondary_dark_dropdown(&self) -> ButtonBuilder<'a> {
        dropdown_button(self.btn_secondary_dark())
    }

    fn btn_popup_light(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_light_dropdown().label_text(text)
    }

    fn btn_popup_dark(&self, text: &'a str) -> ButtonBuilder<'a> {
        self.btn_secondary_dark_dropdown().label_text(text)
    }

    /// A right facing caret, like ">", suitable for paging to the "next" set of results
    fn btn_next(&self) -> ButtonBuilder<'a> {
        self.btn_plain_light_icon("system/assets/tools/next.svg")
            .hotkey(Key::RightArrow)
    }

    /// A left facing caret, like "<", suitable for paging to the "next" set of results
    fn btn_prev(&self) -> ButtonBuilder<'a> {
        self.btn_plain_light_icon("system/assets/tools/prev.svg")
            .hotkey(Key::LeftArrow)
    }

    /// An "X" button to close the current state. Bound to the escape key.
    fn btn_close(&self) -> ButtonBuilder<'a> {
        self.btn_plain_light_icon("system/assets/tools/close.svg")
            .hotkey(Key::Escape)
    }

    /// An "X" button to close the current state. Bound to the escape key and aligned to the right,
    /// usually after a title.
    fn btn_close_widget(&self, ctx: &EventCtx) -> Widget {
        self.btn_close().build_widget(ctx, "close").align_right()
    }

    /// A button which renders it's hotkey for discoverability along with it's label.
    fn btn_hotkey_light(&self, label: &str, key: Key) -> ButtonBuilder<'a>;
}

use crate::{Key, Line, Text};
impl<'a> StyledButtons<'a> for Style {
    fn btn_hotkey_light(&self, label: &str, key: Key) -> ButtonBuilder<'a> {
        let default = {
            let mut txt = Text::new();
            let key_txt = Line(key.describe()).fg(self.hotkey_color);
            txt.append(key_txt);
            let label_text = Line(format!(" - {}", label)).fg(self.btn_primary_light.fg);
            txt.append(label_text);
            txt
        };

        let disabled = {
            let mut txt = Text::new();
            let key_txt = Line(key.describe()).fg(self.hotkey_color.alpha(0.3));
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

    fn btn_plain_destructive(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_destructive;
        plain_builder(colors)
    }

    fn btn_primary_destructive(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_primary_destructive;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }

    fn btn_secondary_destructive(&self) -> ButtonBuilder<'a> {
        let colors = &self.btn_secondary_destructive;
        plain_builder(colors).outline(2.0, colors.outline, ControlState::Default)
    }
}

fn plain_builder<'a>(color_scheme: &ButtonStyle) -> ButtonBuilder<'a> {
    ButtonBuilder::new()
        .label_color(color_scheme.fg, ControlState::Default)
        .label_color(color_scheme.fg_disabled, ControlState::Disabled)
        .image_color(color_scheme.fg, ControlState::Default)
        .image_color(color_scheme.fg_disabled, ControlState::Disabled)
        .bg_color(color_scheme.bg, ControlState::Default)
        .bg_color(color_scheme.bg_hover, ControlState::Hovered)
        .bg_color(color_scheme.bg_disabled, ControlState::Disabled)
}

// Captures some constants for uniform styling of icon-only buttons
fn icon_button<'a>(builder: ButtonBuilder<'a>) -> ButtonBuilder<'a> {
    builder.padding(8.0).image_dims(24.0)
}

fn back_button<'a>(builder: ButtonBuilder<'a>, title: &'a str) -> ButtonBuilder<'a> {
    // DESIGN REVIEW: this button seems absurdly large
    builder
        .image_path("system/assets/pregame/back.svg")
        .label_text(title)
        .padding_left(8.0)
        .font_size(30)
}

fn dropdown_button<'a>(builder: ButtonBuilder<'a>) -> ButtonBuilder<'a> {
    builder
        .image_path("system/assets/tools/arrow_drop_down.svg")
        .image_dims(12.0)
        .stack_spacing(12.0)
        .label_first()
}
