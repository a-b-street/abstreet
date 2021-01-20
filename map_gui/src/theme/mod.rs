pub use widgetry::StyledButtons;
use widgetry::{ButtonBuilder, Key};

// This impl just delegates to the underlying impl on self.gui_style so we can more succinctly write
// `app.cs.btn_primary_dark()` rather than `app.cs.gui_style.btn_primary_dark()`
impl<'a> StyledButtons<'a> for crate::ColorScheme {
    fn btn_primary_dark(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_primary_dark()
    }

    fn btn_secondary_dark(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_secondary_dark()
    }

    fn btn_primary_light(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_primary_light()
    }

    fn btn_secondary_light(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_secondary_light()
    }

    fn btn_plain_dark(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_plain_dark()
    }

    fn btn_plain_light(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_plain_light()
    }

    fn btn_plain_destructive(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_plain_destructive()
    }

    fn btn_primary_destructive(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_primary_destructive()
    }

    fn btn_secondary_destructive(&self) -> ButtonBuilder<'a> {
        self.gui_style.btn_secondary_destructive()
    }

    fn btn_hotkey_light(&self, label: &str, key: Key) -> ButtonBuilder<'a> {
        self.gui_style.btn_hotkey_light(label, key)
    }
}
