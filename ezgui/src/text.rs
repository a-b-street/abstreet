use crate::screen_geom::ScreenRectangle;
use crate::{Canvas, Color, GfxCtx, ScreenPt};
use geom::{Distance, Polygon, Pt2D};
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::glyph_brush::{Section, SectionText, VariedSection};
use nom::types::CompleteStr;
use nom::{alt, char, do_parse, many1, named, separated_pair, take_till1, take_until};
use textwrap;

const FG_COLOR: Color = Color::WHITE;
const BG_COLOR: Color = Color::grey(0.2);
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

    pub fn prompt(line: &str) -> Text {
        let mut txt = Text::new();
        txt.add_styled_line(line.to_string(), None, Some(PROMPT_COLOR), None);
        txt
    }

    pub fn with_bg_color(bg_color: Option<Color>) -> Text {
        Text {
            lines: Vec::new(),
            bg_color,
        }
    }

    pub fn push(&mut self, line: String) {
        self.lines.push((None, Vec::new()));
        parse_style(self, line);
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
                .glyphs
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
        (f64::from(max_width), height)
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
            text: line
                .into_iter()
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
        g.canvas.glyphs.borrow_mut().queue(section);
    }
    g.canvas
        .glyphs
        .borrow_mut()
        .draw_queued(g.display, g.target);

    g.unfork();

    ScreenRectangle {
        x1: top_left.x,
        y1: top_left.y,
        x2: top_left.x + total_width,
        y2: top_left.y + total_height,
    }
}

// TODO Painfully slow at high zooms. Maybe need to use draw_queued_with_transform, expose a
// flush_text_screenspace and flush_text_mapspace.
pub fn draw_text_bubble_mapspace(
    g: &mut GfxCtx,
    top_left: Pt2D,
    txt: &Text,
    // Callers almost always calculate this anyway
    (total_width, total_height): (f64, f64),
) {
    if let Some(c) = txt.bg_color {
        g.draw_polygon(
            c,
            &Polygon::rectangle_topleft(
                top_left,
                Distance::meters(total_width),
                Distance::meters(total_height),
            ),
        );
    }

    let start_at = g
        .canvas
        .map_to_screen(Pt2D::new(top_left.x(), top_left.y()));
    let mut y = 0.0;
    for (line_color, line) in &txt.lines {
        let mut max_size = 0;
        let section = VariedSection {
            screen_position: (start_at.x as f32, (start_at.y + y) as f32),
            text: line
                .into_iter()
                .map(|span| {
                    max_size = max_size.max(span.size);
                    SectionText {
                        text: &span.text,
                        color: span.fg_color.0,
                        scale: Scale::uniform(((span.size as f64) * g.canvas.cam_zoom) as f32),
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
                    Pt2D::new(top_left.x(), top_left.y() + y),
                    Distance::meters(total_width),
                    Distance::meters(height),
                ),
            );
        }

        y += height * g.canvas.cam_zoom;
        g.canvas.glyphs.borrow_mut().queue(section);
    }
    g.canvas
        .glyphs
        .borrow_mut()
        .draw_queued(g.display, g.target);
}

#[derive(Debug)]
struct Append {
    color: Option<Color>,
    text: String,
}

named!(colored<CompleteStr, Append>,
    do_parse!(
        char!('[') >>
        pair: separated_pair!(take_until!(":"), char!(':'), take_until!("]")) >>
        char!(']')
        >>
        (Append {
            color: Some(Color::from_string(&pair.0)),
            text: pair.1.to_string(),
        })
    )
);

fn is_left_bracket(x: char) -> bool {
    x == '['
}

named!(plaintext<CompleteStr, Append>,
    do_parse!(
        txt: take_till1!(is_left_bracket)
        >>
        (Append {
            color: None,
            text: txt.to_string(),
        })
    )
);

named!(chunk<CompleteStr, Append>,
    alt!(colored | plaintext)
);

named!(chunks<CompleteStr, Vec<Append>>,
    many1!(chunk)
);

fn parse_style(txt: &mut Text, line: String) {
    match chunks(CompleteStr(&line)) {
        Ok((rest, values)) => {
            if !rest.is_empty() {
                panic!("Parsing {} had leftover {}", line, rest);
            }
            for x in values {
                txt.append(x.text, x.color);
            }
        }
        x => panic!("Parsing {} broke: {:?}", line, x),
    }
}
