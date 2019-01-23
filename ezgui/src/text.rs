use crate::screen_geom::ScreenRectangle;
use crate::{Canvas, Color, GfxCtx, ScreenPt};
use geom::{Polygon, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::glyph_brush::Section;
use glium_glyph::GlyphBrush;
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.2);
pub const PROMPT_COLOR: Color = Color::BLUE;
pub const SELECTED_COLOR: Color = Color::RED;
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);

const FONT_SIZE: f32 = 24.0;
// TODO These are dependent on FONT_SIZE, but hand-tuned. Glyphs all have 0 as their height, and
// they need adjustments to their positioning.
pub const LINE_HEIGHT: f64 = 32.0;
const MAX_CHAR_WIDTH: f64 = 25.0;

#[derive(Debug, Clone)]
struct TextSpan {
    text: String,
    fg_color: Color,
    // The Text's bg_color will cover the entire block, but some parts can have extra highlighting.
    highlight_color: Option<Color>,
    // TODO bold, italic, font size, font style
}

impl TextSpan {
    fn default_style(text: String) -> TextSpan {
        TextSpan {
            text,
            fg_color: FG_COLOR,
            highlight_color: None,
        }
    }
}

// TODO parse style from markup tags
#[derive(Debug, Clone)]
pub struct Text {
    lines: Vec<Vec<TextSpan>>,
    bg_color: Option<Color>,
}

impl Text {
    pub fn new() -> Text {
        Text {
            lines: Vec::new(),
            bg_color: Some(BG_COLOR),
        }
    }

    pub fn with_bg_color(bg_color: Option<Color>) -> Text {
        Text {
            lines: Vec::new(),
            bg_color,
        }
    }

    pub fn from_line(line: String) -> Text {
        let mut txt = Text::new();
        txt.add_line(line);
        txt
    }

    pub fn pad_if_nonempty(&mut self) {
        if !self.lines.is_empty() {
            self.lines.push(Vec::new());
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push(vec![TextSpan::default_style(line)]);
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
    ) {
        self.lines.push(vec![TextSpan {
            text: line,
            fg_color: fg_color.unwrap_or(FG_COLOR),
            highlight_color,
        }]);
    }

    pub fn append(
        &mut self,
        text: String,
        fg_color: Option<Color>,
        highlight_color: Option<Color>,
    ) {
        if self.lines.is_empty() {
            self.lines.push(Vec::new());
        }

        self.lines.last_mut().unwrap().push(TextSpan {
            text,
            fg_color: fg_color.unwrap_or(FG_COLOR),
            highlight_color,
        });
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn dims(&self, glyphs: &mut GlyphBrush<'static, 'static>) -> (f64, f64) {
        let mut widths: Vec<i32> = Vec::new();
        let mut total_height: i32 = 0;

        for l in &self.lines {
            let full_line = l.iter().fold(String::new(), |mut so_far, span| {
                so_far.push_str(&span.text);
                so_far
            });
            if let Some(rect) = glyphs.pixel_bounds(Section {
                text: &full_line,
                scale: Scale::uniform(FONT_SIZE),
                ..Section::default()
            }) {
                widths.push(rect.width());
                total_height += rect.height();
            } else {
                // TODO Sometimes we want to space something like "    ", but no drawn glyphs
                // means pixel_bounds fails. Hack?
                widths.push((MAX_CHAR_WIDTH * (full_line.len() as f64)) as i32);
                total_height += LINE_HEIGHT as i32;
            }
        }

        (
            widths.into_iter().max().unwrap() as f64,
            total_height as f64,
        )
    }
}

pub fn draw_text_bubble(
    g: &mut GfxCtx,
    glyphs: &mut GlyphBrush<'static, 'static>,
    top_left: ScreenPt,
    txt: Text,
    canvas: &Canvas,
) -> ScreenRectangle {
    // TODO Is it expensive to constantly change uniforms and the shader program?
    g.fork_screenspace(canvas);

    let (total_width, total_height) = txt.dims(glyphs);
    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle_topleft(
                Pt2D::new(top_left.x, top_left.y),
                total_width,
                total_height,
            ),
        );
    }

    let mut y = top_left.y + LINE_HEIGHT;
    for line in &txt.lines {
        let mut x = top_left.x;

        let first_bg_color = line[0].highlight_color;
        let mut same_bg_color = true;
        for (idx, span) in line.into_iter().enumerate() {
            if span.highlight_color != first_bg_color {
                same_bg_color = false;
            }

            let section = Section {
                text: &span.text,
                color: span.fg_color.0,
                scale: Scale::uniform(FONT_SIZE),
                bounds: (1024.0, 768.0),
                screen_position: (x as f32, (y - LINE_HEIGHT) as f32),
                ..Section::default()
            };
            let span_width = glyphs.pixel_bounds(section).unwrap().width() as f64;
            if let Some(color) = span.highlight_color {
                // If this is the last span and all spans use the same background color, then
                // extend the background over the entire width of the text box.
                let width = if idx == line.len() - 1 && same_bg_color {
                    total_width - (x - top_left.x)
                } else {
                    span_width
                };
                g.draw_polygon(
                    color,
                    &Polygon::rectangle_topleft(Pt2D::new(x, y - LINE_HEIGHT), width, LINE_HEIGHT),
                );
            }

            glyphs.queue(section);
            x += span_width;
        }
        y += LINE_HEIGHT;
    }

    g.unfork(canvas);

    ScreenRectangle {
        x1: top_left.x,
        y1: top_left.y,
        x2: top_left.x + total_width,
        y2: top_left.y + total_height,
    }
}
