use crate::assets::Assets;
use crate::{Canvas, Color, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle};
use geom::{Distance, Polygon, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::glyph_brush::{Section, SectionText, VariedSection};
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.3);
pub const PROMPT_COLOR: Color = Color::BLUE;
pub const SELECTED_COLOR: Color = Color::RED;
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);

// TODO Don't do this!
const MAX_CHAR_WIDTH: f64 = 25.0;
pub const SCALE_DOWN: f64 = 10.0;

#[derive(Debug, Clone)]
pub struct TextSpan {
    text: String,
    fg_color: Color,
    size: Option<usize>,
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
}

// TODO What's the better way of doing this? Also "Line" is a bit of a misnomer
#[allow(non_snake_case)]
pub fn Line<S: Into<String>>(text: S) -> TextSpan {
    TextSpan {
        text: text.into(),
        fg_color: FG_COLOR,
        size: None,
    }
}

#[derive(Debug, Clone)]
pub struct Text {
    // The bg_color will cover the entire block, but some lines can have extra highlighting.
    lines: Vec<(Option<Color>, Vec<TextSpan>)>,
    bg_color: Option<Color>,
    pub override_width: Option<f64>,
    pub override_height: Option<f64>,
}

impl Text {
    pub fn new() -> Text {
        Text {
            lines: Vec::new(),
            bg_color: Some(BG_COLOR),
            override_width: None,
            override_height: None,
        }
    }

    pub fn from(line: TextSpan) -> Text {
        let mut txt = Text::new();
        txt.add(line);
        txt
    }

    // TODO nope
    pub fn prompt(line: &str) -> Text {
        let mut txt = Text::new();
        txt.add_highlighted(Line(line), PROMPT_COLOR);
        txt
    }

    pub fn bg(mut self, color: Color) -> Text {
        self.bg_color = Some(color);
        self
    }

    pub fn no_bg(mut self) -> Text {
        self.bg_color = None;
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

    // TODO Ideally we'd wrap last-minute when drawing, but eh, start somewhere.
    pub fn add_wrapped_line(&mut self, canvas: &Canvas, line: String) {
        let wrap_to = canvas.window_width / MAX_CHAR_WIDTH;
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
            height += assets.line_height(max_size);
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
) -> ScreenRectangle {
    // TODO Is it expensive to constantly change uniforms and the shader program?
    g.fork_screenspace();

    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle_topleft(
                top_left.to_pt(),
                Distance::meters(total_dims.width),
                Distance::meters(total_dims.height),
            ),
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
        let height = g.line_height(max_size);

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle_topleft(
                    Pt2D::new(top_left.x, y),
                    Distance::meters(total_dims.width),
                    Distance::meters(height),
                ),
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
            &Polygon::rectangle_topleft(
                top_left,
                Distance::meters(total_dims.width / SCALE_DOWN),
                Distance::meters(total_dims.height / SCALE_DOWN),
            ),
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
        let height = g.line_height(max_size) / SCALE_DOWN;

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle_topleft(
                    Pt2D::new(top_left.x(), y),
                    Distance::meters(total_dims.width / SCALE_DOWN),
                    Distance::meters(height),
                ),
            );
        }

        y += height;
        g.assets.mapspace_glyphs.borrow_mut().queue(section);
    }
}
