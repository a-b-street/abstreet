use crate::assets::Assets;
use crate::{
    svg, Color, EventCtx, GeomBatch, GfxCtx, JustDraw, MultiKey, Prerender, ScreenDims, Widget,
};
use geom::Polygon;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::Hasher;
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.3);
pub const SELECTED_COLOR: Color = Color::grey(0.5);
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);
pub const SCALE_LINE_HEIGHT: f64 = 1.2;

// TODO Don't do this!
pub const MAX_CHAR_WIDTH: f64 = 25.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    Roboto,
    RobotoBold,
    Bungee,
    BungeeInline,
}

#[derive(Debug, Clone)]
pub struct TextSpan {
    text: String,
    fg_color: Color,
    size: Option<usize>,
    font: Font,
}

impl TextSpan {
    pub fn fg(mut self, color: Color) -> TextSpan {
        assert_eq!(self.fg_color, FG_COLOR);
        self.fg_color = color;
        self
    }

    pub fn draw(self, ctx: &EventCtx) -> Widget {
        Text::from(self).draw(ctx)
    }

    // Yuwen's new styles, defined in Figma. Should document them in Github better.

    pub fn display_title(mut self) -> TextSpan {
        self.font = Font::BungeeInline;
        self.size = Some(64);
        self
    }
    pub fn big_heading_styled(mut self) -> TextSpan {
        self.font = Font::Bungee;
        self.size = Some(32);
        self
    }
    pub fn big_heading_plain(mut self) -> TextSpan {
        self.font = Font::RobotoBold;
        self.size = Some(32);
        self
    }
    pub fn small_heading(mut self) -> TextSpan {
        // TODO Roboto medium?
        self.font = Font::RobotoBold;
        self.size = Some(26);
        self
    }
    // The default
    pub fn body(mut self) -> TextSpan {
        self.font = Font::Roboto;
        self.size = Some(21);
        self
    }
    pub fn secondary(mut self) -> TextSpan {
        self.font = Font::Roboto;
        self.size = Some(21);
        self.fg_color = Color::hex("#A3A3A3");
        self
    }
    pub fn small(mut self) -> TextSpan {
        self.font = Font::Roboto;
        self.size = Some(16);
        self
    }
}

// TODO What's the better way of doing this? Also "Line" is a bit of a misnomer
#[allow(non_snake_case)]
pub fn Line<S: Into<String>>(text: S) -> TextSpan {
    TextSpan {
        text: text.into(),
        fg_color: FG_COLOR,
        size: None,
        font: Font::Roboto,
    }
}

#[derive(Debug, Clone)]
pub struct Text {
    // The bg_color will cover the entire block, but some lines can have extra highlighting.
    lines: Vec<(Option<Color>, Vec<TextSpan>)>,
    // TODO Stop using this as much as possible.
    bg_color: Option<Color>,
}

impl Text {
    pub fn new() -> Text {
        Text {
            lines: Vec::new(),
            bg_color: None,
        }
    }

    pub fn from(line: TextSpan) -> Text {
        let mut txt = Text::new();
        txt.add(line);
        txt
    }

    pub fn with_bg(mut self) -> Text {
        assert!(self.bg_color.is_none());
        self.bg_color = Some(BG_COLOR);
        self
    }

    pub fn bg(mut self, bg: Color) -> Text {
        assert!(self.bg_color.is_none());
        self.bg_color = Some(bg);
        self
    }

    // TODO Not exactly sure this is the right place for this, but better than code duplication
    pub fn tooltip(hotkey: Option<MultiKey>, action: &str) -> Text {
        if let Some(ref key) = hotkey {
            let mut txt = Text::from(Line(key.describe()).fg(HOTKEY_COLOR).small());
            txt.append(Line(format!(" - {}", action)));
            txt
        } else {
            Text::from(Line(action).small())
        }
    }

    pub fn change_fg(mut self, fg: Color) -> Text {
        for (_, spans) in self.lines.iter_mut() {
            for span in spans {
                span.fg_color = fg;
            }
        }
        self
    }

    pub fn add(&mut self, line: TextSpan) {
        self.lines.push((None, vec![line]));
    }

    pub fn add_highlighted(&mut self, line: TextSpan, highlight: Color) {
        self.lines.push((Some(highlight), vec![line]));
    }

    // TODO Just one user...
    pub(crate) fn highlight_last_line(&mut self, highlight: Color) {
        self.lines.last_mut().unwrap().0 = Some(highlight);
    }

    pub fn append(&mut self, mut line: TextSpan) {
        if self.lines.is_empty() {
            self.add(line);
            return;
        }

        // Can't override the size mid-line.
        assert_eq!(line.size, None);
        line.size = self
            .lines
            .last()
            .unwrap()
            .1
            .last()
            .map(|span| span.size)
            .unwrap();

        self.lines.last_mut().unwrap().1.push(line);
    }

    pub fn add_appended(&mut self, lines: Vec<TextSpan>) {
        assert!(lines.len() > 1);
        for (idx, l) in lines.into_iter().enumerate() {
            if idx == 0 {
                self.add(l);
            } else {
                self.append(l);
            }
        }
    }

    pub fn append_all(&mut self, lines: Vec<TextSpan>) {
        for l in lines {
            self.append(l);
        }
    }

    pub fn add_wrapped(&mut self, line: String, width: f64) {
        let wrap_to = width / MAX_CHAR_WIDTH;
        for l in textwrap::wrap(&line, wrap_to as usize).into_iter() {
            self.add(Line(l));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn extend(&mut self, other: Text) {
        self.lines.extend(other.lines);
    }

    pub(crate) fn dims(self, assets: &Assets) -> ScreenDims {
        self.render(assets).get_dims()
    }

    pub fn render(self, assets: &Assets) -> GeomBatch {
        self.inner_render(assets, svg::HIGH_QUALITY)
    }

    pub fn render_g(self, g: &GfxCtx) -> GeomBatch {
        self.render(&g.prerender.assets)
    }
    pub fn render_ctx(self, ctx: &EventCtx) -> GeomBatch {
        self.render(&ctx.prerender.assets)
    }

    pub(crate) fn inner_render(self, assets: &Assets, tolerance: f32) -> GeomBatch {
        let hash_key = self.hash_key();
        if let Some(batch) = assets.get_cached_text(&hash_key) {
            return batch;
        }

        let mut output_batch = GeomBatch::new();
        let mut master_batch = GeomBatch::new();

        let mut y = 0.0;
        let mut max_width = 0.0_f64;
        for (line_color, line) in self.lines {
            // Assume size doesn't change mid-line. Always use this fixed line height per font
            // size.
            let line_height = assets.line_height(
                line[0].font,
                line[0].size.unwrap_or(*assets.default_font_size.borrow()),
            );

            let line_batch = render_text(line, tolerance, assets);
            let line_dims = if line_batch.is_empty() {
                ScreenDims::new(0.0, line_height)
            } else {
                // Also lie a little about width to make things look reasonable. TODO Probably
                // should tune based on font size.
                ScreenDims::new(line_batch.get_dims().width + 5.0, line_height)
            };

            if let Some(c) = line_color {
                master_batch.push(
                    c,
                    Polygon::rectangle(line_dims.width, line_dims.height).translate(0.0, y),
                );
            }

            y += line_dims.height;

            // Add all of the padding at the bottom of the line.
            let offset = line_height / SCALE_LINE_HEIGHT * 0.2;
            for (color, poly) in line_batch.consume() {
                master_batch.fancy_push(color, poly.translate(0.0, y - offset));
            }

            max_width = max_width.max(line_dims.width);
        }

        if let Some(c) = self.bg_color {
            output_batch.push(c, Polygon::rectangle(max_width, y));
        }
        for (color, poly) in master_batch.consume() {
            output_batch.fancy_push(color, poly);
        }
        output_batch.dims_text = true;

        assets.cache_text(hash_key, output_batch.clone());
        output_batch
    }

    pub fn render_to_batch(self, prerender: &Prerender) -> GeomBatch {
        let mut batch = self.render(&prerender.assets);
        batch.dims_text = false;
        batch.autocrop()
    }

    fn hash_key(&self) -> String {
        let mut hasher = DefaultHasher::new();
        hasher.write(format!("{:?}", self).as_ref());
        format!("{:x}", hasher.finish())
    }

    pub fn draw(self, ctx: &EventCtx) -> Widget {
        JustDraw::wrap(ctx, self.render_ctx(ctx))
    }
}

fn render_text(spans: Vec<TextSpan>, tolerance: f32, assets: &Assets) -> GeomBatch {
    // TODO This assumes size and font don't change mid-line. We might be able to support that now,
    // actually.

    // Just set a sufficiently large view box
    let mut svg = format!(
        r##"<svg width="9999" height="9999" viewBox="0 0 9999 9999" xmlns="http://www.w3.org/2000/svg"><text x="0" y="0" font-size="{}" {}>"##,
        spans[0].size.unwrap_or(*assets.default_font_size.borrow()),
        match spans[0].font {
            Font::BungeeInline => "font-family=\"Bungee Inline\"",
            Font::Bungee => "font-family=\"Bungee\"",
            Font::Roboto => "font-family=\"Roboto\"",
            Font::RobotoBold => "font-family=\"Roboto\" font-weight=\"bold\"",
        }
    );

    let mut contents = String::new();
    for span in spans {
        write!(
            &mut contents,
            r##"<tspan fill="{}">{}</tspan>"##,
            // TODO Doesn't support alpha
            span.fg_color.to_hex(),
            htmlescape::encode_minimal(&span.text)
        )
        .unwrap();
    }
    write!(&mut svg, "{}</text></svg>", contents).unwrap();

    //println!("- Rendering: {}", contents);

    let svg_tree = match usvg::Tree::from_str(&svg, &assets.text_opts) {
        Ok(t) => t,
        Err(err) => panic!("render_text({}): {}", contents, err),
    };
    let mut batch = GeomBatch::new();
    match crate::svg::add_svg_inner(
        &mut batch,
        svg_tree,
        tolerance,
        *assets.scale_factor.borrow(),
    ) {
        Ok(_) => batch,
        Err(err) => panic!("render_text({}): {}", contents, err),
    }
}

pub trait TextExt {
    fn draw_text(self, ctx: &EventCtx) -> Widget;
}

impl TextExt for &str {
    fn draw_text(self, ctx: &EventCtx) -> Widget {
        Line(self).draw(ctx)
    }
}

impl TextExt for String {
    fn draw_text(self, ctx: &EventCtx) -> Widget {
        Line(self).draw(ctx)
    }
}
