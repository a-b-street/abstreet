use crate::screen_geom::ScreenRectangle;
use crate::{Canvas, Color, GfxCtx, ScreenPt};
use geom::{Distance, Polygon, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::glyph_brush::{Section, SectionText, VariedSection};
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.2);
pub const PROMPT_COLOR: Color = Color::BLUE;
pub const SELECTED_COLOR: Color = Color::RED;
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);

pub const FONT_SIZE: usize = 30;
// TODO Don't do this!
const MAX_CHAR_WIDTH: f64 = 25.0;
pub const SCALE_DOWN: f64 = 10.0;

#[derive(Debug, Clone)]
struct TextSpan {
    text: String,
    fg_color: Color,
    size: usize,
    // TODO bold, italic, font style
}

impl TextSpan {
    fn default_style(text: String) -> TextSpan {
        TextSpan {
            text,
            fg_color: FG_COLOR,
            size: FONT_SIZE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Text {
    // The bg_color will cover the entire block, but some lines can have extra highlighting.
    lines: Vec<(Option<Color>, Vec<TextSpan>)>,
    bg_color: Option<Color>,
    pub(crate) override_width: Option<f64>,
    pub(crate) override_height: Option<f64>,
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

    pub fn prompt(line: &str) -> Text {
        let mut txt = Text::new();
        txt.add_styled_line(line.to_string(), None, Some(PROMPT_COLOR), None);
        txt
    }

    pub fn with_bg_color(bg_color: Option<Color>) -> Text {
        Text {
            lines: Vec::new(),
            bg_color,
            override_width: None,
            override_height: None,
        }
    }

    pub fn from_line(line: String) -> Text {
        let mut txt = Text::new();
        txt.add_line(line);
        txt
    }

    pub fn from_styled_line(
        line: String,
        fg_color: Option<Color>,
        highlight_color: Option<Color>,
        font_size: Option<usize>,
    ) -> Text {
        let mut txt = Text::new();
        txt.add_styled_line(line, fg_color, highlight_color, font_size);
        txt
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push((None, vec![TextSpan::default_style(line)]));
    }

    // TODO Ideally we'd wrap last-minute when drawing, but eh, start somewhere.
    pub fn add_wrapped_line(&mut self, canvas: &Canvas, line: String) {
        let wrap_to = canvas.window_width / MAX_CHAR_WIDTH;
        for l in textwrap::wrap(&line, wrap_to as usize).into_iter() {
            self.add_line(l.to_string());
        }
    }

    pub fn add_styled_line(
        &mut self,
        line: String,
        fg_color: Option<Color>,
        highlight_color: Option<Color>,
        font_size: Option<usize>,
    ) {
        self.lines.push((
            highlight_color,
            vec![TextSpan {
                text: line,
                fg_color: fg_color.unwrap_or(FG_COLOR),
                size: font_size.unwrap_or(FONT_SIZE),
            }],
        ));
    }

    pub fn append(&mut self, text: String, fg_color: Option<Color>) {
        if self.lines.is_empty() {
            self.lines.push((None, Vec::new()));
        }

        let size = self
            .lines
            .last()
            .unwrap()
            .1
            .last()
            .map(|span| span.size)
            .unwrap_or(FONT_SIZE);
        self.lines.last_mut().unwrap().1.push(TextSpan {
            text,
            fg_color: fg_color.unwrap_or(FG_COLOR),
            size,
        });
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub fn extend(&mut self, other: &Text) {
        self.lines.extend(other.lines.clone())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn dims(&self, canvas: &Canvas) -> (f64, f64) {
        let mut max_width = 0;
        let mut height = 0.0;

        for (_, line) in &self.lines {
            let mut full_line = String::new();
            let mut max_size = 0;
            for span in line {
                full_line.push_str(&span.text);
                max_size = max_size.max(span.size);
            }
            // Empty lines or whitespace-only lines effectively have 0 width.
            let width = canvas
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
            height += canvas.line_height(max_size);
        }
        (
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
    (total_width, total_height): (f64, f64),
) -> ScreenRectangle {
    // TODO Is it expensive to constantly change uniforms and the shader program?
    g.fork_screenspace();

    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle_topleft(
                Pt2D::new(top_left.x, top_left.y),
                Distance::meters(total_width),
                Distance::meters(total_height),
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
                    max_size = max_size.max(span.size);
                    SectionText {
                        text: &span.text,
                        color: span.fg_color.0,
                        scale: Scale::uniform(span.size as f32),
                        ..SectionText::default()
                    }
                })
                .collect(),
            ..VariedSection::default()
        };
        let height = g.canvas.line_height(max_size);

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle_topleft(
                    Pt2D::new(top_left.x, y),
                    Distance::meters(total_width),
                    Distance::meters(height),
                ),
            );
        }

        y += height;
        g.canvas.screenspace_glyphs.borrow_mut().queue(section);
    }

    g.unfork();

    ScreenRectangle {
        x1: top_left.x,
        y1: top_left.y,
        x2: top_left.x + total_width,
        y2: top_left.y + total_height,
    }
}

pub fn draw_text_bubble_mapspace(
    g: &mut GfxCtx,
    top_left: Pt2D,
    txt: &Text,
    // Callers almost always calculate this anyway
    (total_width, total_height): (f64, f64),
) {
    // TODO If this works, share most code with draw_text_bubble.
    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle_topleft(
                Pt2D::new(top_left.x(), top_left.y()),
                Distance::meters(total_width / SCALE_DOWN),
                Distance::meters(total_height / SCALE_DOWN),
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
                    max_size = max_size.max(span.size);
                    SectionText {
                        text: &span.text,
                        color: span.fg_color.0,
                        scale: Scale::uniform(span.size as f32),
                        ..SectionText::default()
                    }
                })
                .collect(),
            ..VariedSection::default()
        };
        let height = g.canvas.line_height(max_size) / SCALE_DOWN;

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle_topleft(
                    Pt2D::new(top_left.x(), y),
                    Distance::meters(total_width / SCALE_DOWN),
                    Distance::meters(height),
                ),
            );
        }

        y += height;
        g.canvas.mapspace_glyphs.borrow_mut().queue(section);
    }
}
