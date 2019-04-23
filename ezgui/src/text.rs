use crate::screen_geom::ScreenRectangle;
use crate::{Canvas, Color, GfxCtx, ScreenPt};
use geom::{Polygon, Pt2D};
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

// TODO parse style from markup tags
#[derive(Debug, Clone)]
pub struct Text {
    // The bg_color will cover the entire block, but some lines can have extra highlighting.
    lines: Vec<(Option<Color>, Vec<TextSpan>)>,
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
            self.lines.push((None, Vec::new()));
        }
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

        self.lines.last_mut().unwrap().1.push(TextSpan {
            text,
            fg_color: fg_color.unwrap_or(FG_COLOR),
            size: FONT_SIZE,
        });
    }

    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn dims(&self, canvas: &Canvas) -> (f64, f64) {
        // Always use the max height, since other stuff like menus assume a fixed height.
        let height = (self.lines.len() as f64) * canvas.line_height(FONT_SIZE);

        let mut glyphs = canvas.glyphs.borrow_mut();
        let width = f64::from(
            self.lines
                .iter()
                .map(|(_, l)| {
                    let full_line = l.iter().fold(String::new(), |mut so_far, span| {
                        so_far.push_str(&span.text);
                        so_far
                    });
                    // Empty lines or whitespace-only lines effectively have 0 width.
                    glyphs
                        .pixel_bounds(Section {
                            text: &full_line,
                            scale: Scale::uniform(FONT_SIZE as f32),
                            ..Section::default()
                        })
                        .map(|rect| rect.width())
                        .unwrap_or(0)
                })
                .max()
                .unwrap(),
        );

        (width, height)
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
                total_width,
                total_height,
            ),
        );
    }

    let mut y = top_left.y;
    for (line_color, line) in &txt.lines {
        let section = VariedSection {
            screen_position: (top_left.x as f32, y as f32),
            text: line
                .into_iter()
                .map(|span| SectionText {
                    text: &span.text,
                    color: span.fg_color.0,
                    scale: Scale::uniform(FONT_SIZE as f32),
                    ..SectionText::default()
                })
                .collect(),
            ..VariedSection::default()
        };

        if let Some(c) = line_color {
            g.draw_polygon(
                *c,
                &Polygon::rectangle_topleft(
                    Pt2D::new(top_left.x, y),
                    total_width,
                    g.canvas.line_height(FONT_SIZE),
                ),
            );
        }

        y += g.canvas.line_height(FONT_SIZE);
        g.canvas.glyphs.borrow_mut().queue(section);
    }

    g.unfork();

    ScreenRectangle {
        x1: top_left.x,
        y1: top_left.y,
        x2: top_left.x + total_width,
        y2: top_left.y + total_height,
    }
}
