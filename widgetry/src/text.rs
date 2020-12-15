use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::Hasher;

use geom::{PolyLine, Polygon};

use crate::assets::Assets;
use crate::{
    svg, Color, DeferDraw, EventCtx, GeomBatch, JustDraw, MultiKey, Prerender, ScreenDims, Widget,
};

// Same as body()
pub const DEFAULT_FONT: Font = Font::OverpassRegular;
pub const DEFAULT_FONT_SIZE: usize = 21;
const DEFAULT_FG_COLOR: Color = Color::WHITE;

pub const BG_COLOR: Color = Color::grey(0.3);
pub const SELECTED_COLOR: Color = Color::grey(0.5);
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.8);
pub const SCALE_LINE_HEIGHT: f64 = 1.2;

// TODO Almost gone!
pub const MAX_CHAR_WIDTH: f64 = 25.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    BungeeInlineRegular,
    BungeeRegular,
    OverpassBold,
    OverpassRegular,
    OverpassSemiBold,
    OverpassMonoBold,
    ZcoolXiaoWei,
}

impl Font {
    pub fn family(self) -> &'static str {
        match self {
            Font::BungeeInlineRegular => "Bungee Inline",
            Font::BungeeRegular => "Bungee",
            Font::OverpassBold => "Overpass",
            Font::OverpassRegular => "Overpass",
            Font::OverpassSemiBold => "Overpass",
            Font::OverpassMonoBold => "Overpass Mono",
            Font::ZcoolXiaoWei => "ZCOOL XiaoWei",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextSpan {
    text: String,
    fg_color: Color,
    size: usize,
    font: Font,
    underlined: bool,
}

impl TextSpan {
    pub fn fg(mut self, color: Color) -> TextSpan {
        assert_eq!(self.fg_color, DEFAULT_FG_COLOR);
        self.fg_color = color;
        self
    }
    pub fn maybe_fg(mut self, color: Option<Color>) -> TextSpan {
        assert_eq!(self.fg_color, DEFAULT_FG_COLOR);
        if let Some(c) = color {
            self.fg_color = c;
        }
        self
    }

    pub fn draw(self, ctx: &EventCtx) -> Widget {
        Text::from(self).draw(ctx)
    }
    pub fn batch(self, ctx: &EventCtx) -> Widget {
        Text::from(self).batch(ctx)
    }

    // Yuwen's new styles, defined in Figma. Should document them in Github better.

    pub fn display_title(mut self) -> TextSpan {
        self.font = Font::BungeeInlineRegular;
        self.size = 64;
        self
    }
    pub fn big_heading_styled(mut self) -> TextSpan {
        self.font = Font::BungeeRegular;
        self.size = 32;
        self
    }
    pub fn big_heading_plain(mut self) -> TextSpan {
        self.font = Font::OverpassBold;
        self.size = 32;
        self
    }
    pub fn small_heading(mut self) -> TextSpan {
        self.font = Font::OverpassSemiBold;
        self.size = 26;
        self
    }
    // The default
    pub fn body(mut self) -> TextSpan {
        self.font = Font::OverpassRegular;
        self.size = 21;
        self
    }
    pub fn secondary(mut self) -> TextSpan {
        self.font = Font::OverpassRegular;
        self.size = 21;
        self.fg_color = Color::hex("#A3A3A3");
        self
    }
    pub fn small(mut self) -> TextSpan {
        self.font = Font::OverpassRegular;
        self.size = 16;
        self
    }
    pub fn big_monospaced(mut self) -> TextSpan {
        self.font = Font::OverpassMonoBold;
        self.size = 32;
        self
    }
    pub fn small_monospaced(mut self) -> TextSpan {
        self.font = Font::OverpassMonoBold;
        self.size = 16;
        self
    }

    pub fn underlined(mut self) -> TextSpan {
        self.underlined = true;
        self
    }
}

// TODO What's the better way of doing this? Also "Line" is a bit of a misnomer
#[allow(non_snake_case)]
pub fn Line<S: Into<String>>(text: S) -> TextSpan {
    TextSpan {
        text: text.into(),
        fg_color: DEFAULT_FG_COLOR,
        size: DEFAULT_FONT_SIZE,
        font: DEFAULT_FONT,
        underlined: false,
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

    pub fn from_all(lines: Vec<TextSpan>) -> Text {
        let mut txt = Text::new();
        for l in lines {
            txt.append(l);
        }
        txt
    }

    pub fn from_multiline(lines: Vec<TextSpan>) -> Text {
        let mut txt = Text::new();
        for l in lines {
            txt.add(l);
        }
        txt
    }

    // TODO Remove this
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
    pub fn tooltip<MK: Into<Option<MultiKey>>>(ctx: &EventCtx, hotkey: MK, action: &str) -> Text {
        if let Some(ref key) = hotkey.into() {
            Text::from_all(vec![
                Line(key.describe()).fg(ctx.style().hotkey_color).small(),
                Line(format!(" - {}", action)).small(),
            ])
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

    pub fn append(&mut self, line: TextSpan) {
        if self.lines.is_empty() {
            self.add(line);
            return;
        }

        // Can't override the size or font mid-line.
        let last = self.lines.last().unwrap().1.last().unwrap();
        assert_eq!(line.size, last.size);
        assert_eq!(line.font, last.font);

        self.lines.last_mut().unwrap().1.push(line);
    }

    pub fn add_appended(&mut self, lines: Vec<TextSpan>) {
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

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn extend(&mut self, other: Text) {
        self.lines.extend(other.lines);
    }

    pub(crate) fn dims(self, assets: &Assets) -> ScreenDims {
        self.render(assets).get_dims()
    }

    /// Render the text, without any autocropping. You can pass in an `EventCtx` or `GfxCtx`.
    pub fn render<A: AsRef<Assets>>(self, assets: &A) -> GeomBatch {
        let assets: &Assets = assets.as_ref();
        self.inner_render(assets, svg::HIGH_QUALITY)
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
            let line_height = assets.line_height(line[0].font, line[0].size);

            let line_batch = render_line(line, tolerance, assets);
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
            master_batch.append(line_batch.translate(0.0, y - offset));

            max_width = max_width.max(line_dims.width);
        }

        if let Some(c) = self.bg_color {
            output_batch.push(c, Polygon::rectangle(max_width, y));
        }
        output_batch.append(master_batch);
        output_batch.autocrop_dims = false;

        assets.cache_text(hash_key, output_batch.clone());
        output_batch
    }

    /// Render the text, autocropping blank space out of the result. You can pass in an `EventCtx`
    /// or `GfxCtx`.
    pub fn render_autocropped<A: AsRef<Assets>>(self, assets: &A) -> GeomBatch {
        let mut batch = self.render(assets);
        batch.autocrop_dims = true;
        batch.autocrop()
    }

    fn hash_key(&self) -> String {
        let mut hasher = DefaultHasher::new();
        hasher.write(format!("{:?}", self).as_ref());
        format!("{:x}", hasher.finish())
    }

    pub fn draw(self, ctx: &EventCtx) -> Widget {
        JustDraw::wrap(ctx, self.render(ctx))
    }
    pub fn batch(self, ctx: &EventCtx) -> Widget {
        DeferDraw::new(self.render(ctx))
    }

    pub fn wrap_to_pct(self, ctx: &EventCtx, pct: usize) -> Text {
        self.inner_wrap_to_pct(
            (pct as f64) / 100.0 * ctx.canvas.window_width,
            &ctx.prerender.assets,
        )
    }

    pub(crate) fn inner_wrap_to_pct(mut self, limit: f64, assets: &Assets) -> Text {
        let mut lines = Vec::new();
        for (bg, spans) in self.lines.drain(..) {
            // First optimistically assume everything just fits.
            if render_line(spans.clone(), svg::LOW_QUALITY, assets)
                .get_dims()
                .width
                < limit
            {
                lines.push((bg, spans));
                continue;
            }

            // Greedy approach, fit as many words on a line as possible. Don't do all of that
            // hyphenation nonsense.
            let mut width_left = limit;
            let mut current_line = Vec::new();
            for span in spans {
                let mut current_span = span.clone();
                current_span.text = String::new();
                for word in span.text.split_whitespace() {
                    let width = render_line(
                        vec![TextSpan {
                            text: word.to_string(),
                            size: span.size,
                            font: span.font,
                            fg_color: span.fg_color,
                            underlined: false,
                        }],
                        svg::LOW_QUALITY,
                        assets,
                    )
                    .get_dims()
                    .width;
                    if width_left > width {
                        current_span.text.push(' ');
                        current_span.text.push_str(word);
                        width_left -= width;
                    } else {
                        current_line.push(current_span);
                        lines.push((bg, current_line.drain(..).collect()));

                        current_span = span.clone();
                        current_span.text = word.to_string();
                        width_left = limit;
                    }
                }
                if !current_span.text.is_empty() {
                    current_line.push(current_span);
                }
            }
            if !current_line.is_empty() {
                lines.push((bg, current_line));
            }
        }
        self.lines = lines;
        self
    }
}

fn render_line(spans: Vec<TextSpan>, tolerance: f32, assets: &Assets) -> GeomBatch {
    // TODO This assumes size and font don't change mid-line. We might be able to support that now,
    // actually.
    // https://www.oreilly.com/library/view/svg-text-layout/9781491933817/ch04.html

    // Just set a sufficiently large view box
    let mut svg = r##"<svg width="9999" height="9999" viewBox="0 0 9999 9999" xmlns="http://www.w3.org/2000/svg">"##.to_string();

    write!(
        &mut svg,
        r##"<text x="0" y="0" xml:space="preserve" font-size="{}" font-family="{}" {}>"##,
        spans[0].size,
        spans[0].font.family(),
        match spans[0].font {
            Font::OverpassBold => "font-weight=\"bold\"",
            Font::OverpassSemiBold => "font-weight=\"600\"",
            _ => "",
        }
    )
    .unwrap();

    let mut contents = String::new();
    for span in spans {
        write!(
            &mut contents,
            r##"<tspan fill="{}" {}>{}</tspan>"##,
            // TODO Doesn't support alpha
            span.fg_color.to_hex(),
            if span.underlined {
                "text-decoration=\"underline\""
            } else {
                ""
            },
            htmlescape::encode_minimal(&span.text)
        )
        .unwrap();
    }
    write!(&mut svg, "{}</text></svg>", contents).unwrap();

    let svg_tree = match usvg::Tree::from_str(&svg, &assets.text_opts) {
        Ok(t) => t,
        Err(err) => panic!("render_line({}): {}", contents, err),
    };
    let mut batch = GeomBatch::new();
    match crate::svg::add_svg_inner(&mut batch, svg_tree, tolerance) {
        Ok(_) => batch,
        Err(err) => panic!("render_line({}): {}", contents, err),
    }
}

pub trait TextExt {
    fn draw_text(self, ctx: &EventCtx) -> Widget;
    fn batch_text(self, ctx: &EventCtx) -> Widget;
}

impl TextExt for &str {
    fn draw_text(self, ctx: &EventCtx) -> Widget {
        Line(self).draw(ctx)
    }
    fn batch_text(self, ctx: &EventCtx) -> Widget {
        Line(self).batch(ctx)
    }
}

impl TextExt for String {
    fn draw_text(self, ctx: &EventCtx) -> Widget {
        Line(self).draw(ctx)
    }
    fn batch_text(self, ctx: &EventCtx) -> Widget {
        Line(self).batch(ctx)
    }
}

impl TextSpan {
    // TODO Copies from render_line a fair amount
    pub fn render_curvey(self, prerender: &Prerender, path: &PolyLine, scale: f64) -> GeomBatch {
        let assets = &prerender.assets;
        let tolerance = svg::HIGH_QUALITY;

        // Just set a sufficiently large view box
        let mut svg = r##"<svg width="9999" height="9999" viewBox="0 0 9999 9999" xmlns="http://www.w3.org/2000/svg">"##.to_string();

        write!(
            &mut svg,
            r##"<path id="txtpath" fill="none" stroke="none" d=""##
        )
        .unwrap();
        write!(
            &mut svg,
            "M {} {}",
            path.points()[0].x(),
            path.points()[0].y()
        )
        .unwrap();
        for pt in path.points().into_iter().skip(1) {
            write!(&mut svg, " L {} {}", pt.x(), pt.y()).unwrap();
        }
        write!(&mut svg, "\" />").unwrap();
        // We need to subtract and account for the length of the text
        let start_offset = (path.length() / 2.0).inner_meters()
            - (Text::from(Line(&self.text)).dims(assets).width * scale) / 2.0;
        write!(
            &mut svg,
            r##"<text xml:space="preserve" font-size="{}" font-family="{}" {} fill="{}" startOffset="{}">"##,
            // This is seemingly the easiest way to do this. We could .scale() the whole batch
            // after, but then we have to re-translate it to the proper spot
            (self.size as f64) * scale,
            self.font.family(),
            match self.font {
                Font::OverpassBold => "font-weight=\"bold\"",
                Font::OverpassSemiBold => "font-weight=\"600\"",
                _ => "",
            },
            self.fg_color.to_hex(),
            start_offset,
        )
            .unwrap();

        write!(
            &mut svg,
            r##"<textPath href="#txtpath">{}</textPath></text></svg>"##,
            self.text
        )
        .unwrap();

        let svg_tree = match usvg::Tree::from_str(&svg, &assets.text_opts) {
            Ok(t) => t,
            Err(err) => panic!("curvey({}): {}", self.text, err),
        };
        let mut batch = GeomBatch::new();
        match crate::svg::add_svg_inner(&mut batch, svg_tree, tolerance) {
            Ok(_) => batch,
            Err(err) => panic!("curvey({}): {}", self.text, err),
        }
    }
}
