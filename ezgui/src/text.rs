use crate::assets::Assets;
use crate::{Color, GeomBatch, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle};
use geom::{Polygon, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::{FontId, GlyphCruncher};
use glium_glyph::glyph_brush::{Section, SectionText, VariedSection};
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.3);
pub const PROMPT_COLOR: Color = Color::BLUE;
pub const SELECTED_COLOR: Color = Color::grey(0.5);
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);

// TODO Don't do this!
const MAX_CHAR_WIDTH: f64 = 25.0;
pub const SCALE_DOWN: f64 = 60.0;

// These're hardcoded for simplicity; this list doesn't change much.
const DEJA_VU: FontId = FontId(0);
const ROBOTO: FontId = FontId(1);
const ROBOTO_BOLD: FontId = FontId(2);

#[derive(Debug, Clone)]
pub struct TextSpan {
    text: String,
    fg_color: Color,
    size: Option<usize>,
    font: FontId,
    // TODO bold, italic, font style
}

impl TextSpan {
    pub fn fg(mut self, color: Color) -> TextSpan {
        assert_eq!(self.fg_color, FG_COLOR);
        self.fg_color = color;
        self
    }

    pub fn size(mut self, size: usize) -> TextSpan {
        assert_eq!(self.size, None);
        self.size = Some(size);
        self
    }

    pub fn roboto(mut self) -> TextSpan {
        assert_eq!(self.font, DEJA_VU);
        self.font = ROBOTO;
        self
    }

    pub fn roboto_bold(mut self) -> TextSpan {
        assert_eq!(self.font, DEJA_VU);
        self.font = ROBOTO_BOLD;
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
        font: DEJA_VU,
    }
}

#[derive(Debug, Clone)]
pub struct Text {
    // The bg_color will cover the entire block, but some lines can have extra highlighting.
    lines: Vec<(Option<Color>, Vec<TextSpan>)>,
    bg_color: Option<Color>,
    // TODO Definitely a hack to replace with Composite.
    pub override_width: Option<f64>,
    pub override_height: Option<f64>,
}

impl Text {
    pub fn new() -> Text {
        Text {
            lines: Vec::new(),
            bg_color: None,
            override_width: None,
            override_height: None,
        }
    }

    pub fn from(line: TextSpan) -> Text {
        let mut txt = Text::new();
        txt.add(line);
        txt
    }

    pub fn prompt(line: &str) -> Text {
        let mut txt = Text::new().with_bg();
        txt.add_highlighted(Line(line), PROMPT_COLOR);
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

    pub fn highlight_last_line(&mut self, highlight: Color) {
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

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub fn extend(&mut self, other: &Text) {
        self.lines.extend(other.lines.clone())
    }

    pub(crate) fn dims(&self, assets: &Assets) -> ScreenDims {
        let mut max_width = 0;
        let mut height = 0.0;

        for (_, line) in &self.lines {
            let mut full_line = String::new();
            let mut max_size = 0;
            for span in line {
                full_line.push_str(&span.text);
                max_size = max_size.max(span.size.unwrap_or(assets.font_size));
            }
            // Empty lines or whitespace-only lines effectively have 0 width.
            let width = assets
                .screenspace_glyphs
                .borrow_mut()
                .pixel_bounds(Section {
                    text: &full_line,
                    scale: Scale::uniform(max_size as f32),
                    ..Section::default()
                })
                .map(|rect| rect.width())
                .unwrap_or(0);
            max_width = max_width.max(width);
            // TODO Assume the same font for all spans
            height += assets.line_height(line[0].font, max_size);
        }
        ScreenDims::new(
            self.override_width.unwrap_or_else(|| f64::from(max_width)),
            self.override_height.unwrap_or_else(|| height),
        )
    }
}

pub fn draw_text_bubble(
    g: &mut GfxCtx,
    top_left: ScreenPt,
    txt: &Text,
    // Callers almost always calculate this anyway
    total_dims: ScreenDims,
    defer_clipping: bool,
) -> ScreenRectangle {
    // TODO Such nonsense.
    if g.params.scissor.is_some() && defer_clipping {
        let mut defer = g.assets.screenspace_clip_glyphs.borrow_mut();
        if let Some((ref rect, ref mut list)) = *defer {
            assert_eq!(rect, g.params.scissor.as_ref().unwrap());
            list.push((top_left, txt.clone(), total_dims));
        } else {
            *defer = Some((
                g.params.scissor.clone().unwrap(),
                vec![(top_left, txt.clone(), total_dims)],
            ));
        }
        return ScreenRectangle::top_left(top_left, total_dims);
    }

    // TODO Is it expensive to constantly change uniforms and the shader program?
    g.fork_screenspace();

    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle(total_dims.width, total_dims.height)
                .translate(top_left.x, top_left.y),
        );
    }

    let mut y = top_left.y;
    for (line_color, line) in &txt.lines {
        let mut max_size = 0;
        let section = VariedSection {
            screen_position: (top_left.x as f32, y as f32),
            z: 0.5,
            text: line
                .iter()
                .map(|span| {
                    max_size = max_size.max(span.size.unwrap_or(g.assets.font_size));
                    SectionText {
                        text: &span.text,
                        color: match span.fg_color {
                            Color::RGBA(r, g, b, a) => [r, g, b, a],
                            _ => unreachable!(),
                        },
                        scale: Scale::uniform(span.size.unwrap_or(g.assets.font_size) as f32),
                        ..SectionText::default()
                    }
                })
                .collect(),
            ..VariedSection::default()
        };
        // TODO Assume the same font for all spans
        let height = g.line_height(line[0].font, max_size);

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle(total_dims.width, height).translate(top_left.x, y),
            );
        }

        y += height;
        g.assets.screenspace_glyphs.borrow_mut().queue(section);
    }

    g.unfork();

    ScreenRectangle::top_left(top_left, total_dims)
}

pub fn draw_text_bubble_mapspace(
    g: &mut GfxCtx,
    top_left: Pt2D,
    txt: &Text,
    // Callers almost always calculate this anyway
    total_dims: ScreenDims,
) {
    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle(
                total_dims.width / SCALE_DOWN,
                total_dims.height / SCALE_DOWN,
            )
            .translate(top_left.x(), top_left.y()),
        );
    }

    let mut y = top_left.y();
    for (line_color, line) in &txt.lines {
        let mut max_size = 0;
        let section = VariedSection {
            // This in map-space, but the transform matrix for mapspace_glyphs will take care of
            // it.
            screen_position: ((top_left.x() * SCALE_DOWN) as f32, (y * SCALE_DOWN) as f32),
            z: 0.1,
            text: line
                .iter()
                .map(|span| {
                    max_size = max_size.max(span.size.unwrap_or(g.assets.font_size));
                    SectionText {
                        text: &span.text,
                        color: match span.fg_color {
                            Color::RGBA(r, g, b, a) => [r, g, b, a],
                            _ => unreachable!(),
                        },
                        scale: Scale::uniform(span.size.unwrap_or(g.assets.font_size) as f32),
                        ..SectionText::default()
                    }
                })
                .collect(),
            ..VariedSection::default()
        };
        // TODO Assume the same font for all spans
        let height = g.line_height(line[0].font, max_size) / SCALE_DOWN;

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle(total_dims.width / SCALE_DOWN, height)
                    .translate(top_left.x(), y),
            );
        }

        y += height;
        g.assets.mapspace_glyphs.borrow_mut().queue(section);
    }
}

// TODO Rearrange
impl Text {
    pub fn render(self, master_batch: &mut GeomBatch, top_left: ScreenPt) -> ScreenRectangle {
        /*if let Some(c) = txt.bg_color {
            g.draw_polygon(
                c,
                &Polygon::rectangle(total_dims.width, total_dims.height)
                    .translate(top_left.x, top_left.y),
            );
        }*/

        let mut y = top_left.y;
        let mut max_width = 0.0_f64;
        for (line_color, line) in self.lines {
            let mut x = top_left.x;
            let mut line_batch = GeomBatch::new();
            for piece in line {
                let mini_batch = render_text(piece);
                let dims = mini_batch.get_dims();
                for (color, poly) in mini_batch.consume() {
                    line_batch.push(color, poly.translate(x, y));
                }
                x += dims.width;
            }
            let line_dims = line_batch.get_dims();

            if let Some(c) = line_color {
                master_batch.push(
                    c,
                    Polygon::rectangle(x - top_left.x, line_dims.height).translate(top_left.x, y),
                );
            }

            for (color, poly) in line_batch.consume() {
                master_batch.push(color, poly.translate(0.0, line_dims.height));
            }

            y += line_dims.height;
            max_width = max_width.max(x - top_left.x);
        }

        ScreenRectangle::top_left(top_left, ScreenDims::new(max_width, y - top_left.y))
    }
}

fn render_text(txt: TextSpan) -> GeomBatch {
    // If these are large enough, does this work?
    let max_w = 9999.0;
    let max_h = 9999.0;

    let svg = format!(
        r##"<svg width="{}" height="{}" viewBox="0 0 {} {}" fill="none" xmlns="http://www.w3.org/2000/svg"><text x="0" y="0" fill="{}" font-size="{}" font-family="{}">{}</text></svg>"##,
        max_w,
        max_h,
        max_w,
        max_h,
        txt.fg_color.to_hex(),
        // TODO Plumb through default font size?
        txt.size.unwrap_or(30),
        // TODO Make these work
        "DejaVu Sans",
        txt.text
    );

    let svg_tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
    let mut batch = GeomBatch::new();
    match crate::svg::add_svg_inner(&mut batch, svg_tree) {
        Ok(_) => batch,
        Err(err) => panic!("render_text({}): {}", txt.text, err),
    }
}
