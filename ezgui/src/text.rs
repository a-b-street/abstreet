use crate::assets::Assets;
use crate::{Color, GeomBatch, Prerender, ScreenDims};
use geom::Polygon;
use std::fmt::Write;
use textwrap;

const FG_COLOR: Color = Color::WHITE;
pub const BG_COLOR: Color = Color::grey(0.3);
pub const PROMPT_COLOR: Color = Color::BLUE;
pub const SELECTED_COLOR: Color = Color::grey(0.5);
pub const HOTKEY_COLOR: Color = Color::GREEN;
pub const INACTIVE_CHOICE_COLOR: Color = Color::grey(0.4);

// TODO Don't do this!
const MAX_CHAR_WIDTH: f64 = 25.0;

#[derive(Debug, Clone, PartialEq)]
enum Font {
    DejaVu,
    Roboto,
    RobotoBold,
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

    pub fn size(mut self, size: usize) -> TextSpan {
        assert_eq!(self.size, None);
        self.size = Some(size);
        self
    }

    pub fn roboto(mut self) -> TextSpan {
        assert_eq!(self.font, Font::DejaVu);
        self.font = Font::Roboto;
        self
    }

    pub fn roboto_bold(mut self) -> TextSpan {
        assert_eq!(self.font, Font::DejaVu);
        self.font = Font::RobotoBold;
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
        font: Font::DejaVu,
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

    pub(crate) fn dims(self, assets: &Assets) -> ScreenDims {
        // TODO Still pay attention to this hack, so the loading screen isn't dreadfully slow
        if let Some(w) = self.override_width {
            return ScreenDims::new(w, self.override_height.unwrap());
        }
        self.render(assets).get_dims()
    }

    pub fn render(self, _: &Assets) -> GeomBatch {
        // TODO Bad guess
        let empty_line_height = 30.0;

        let mut output_batch = GeomBatch::new();
        let mut master_batch = GeomBatch::new();

        let mut y = 0.0;
        let mut max_width = 0.0_f64;
        for (line_color, line) in self.lines {
            let line_batch = render_text(line);
            let line_dims = if line_batch.is_empty() {
                ScreenDims::new(0.0, empty_line_height)
            } else {
                line_batch.get_dims()
            };

            if let Some(c) = line_color {
                master_batch.push(
                    c,
                    Polygon::rectangle(line_dims.width, line_dims.height).translate(0.0, y),
                );
            }

            // TODO Do this first or not? Should we call realign() around here?
            y += line_dims.height;

            for (color, poly) in line_batch.consume() {
                master_batch.push(color, poly.translate(0.0, y));
            }

            max_width = max_width.max(line_dims.width);
        }

        if let Some(c) = self.bg_color {
            output_batch.push(c, Polygon::rectangle(max_width, y));
        }
        for (color, poly) in master_batch.consume() {
            output_batch.push(color, poly);
        }

        output_batch
    }

    pub fn render_to_batch(self, prerender: &Prerender) -> GeomBatch {
        self.render(&prerender.assets).realign()
    }
}

fn render_text(spans: Vec<TextSpan>) -> GeomBatch {
    // TODO This assumes size and font don't change mid-line. We might be able to support that now,
    // actually.

    // Just set a sufficiently large view box
    let mut svg = format!(
        r##"<svg width="9999" height="9999" viewBox="0 0 9999 9999" xmlns="http://www.w3.org/2000/svg"><text x="0" y="0" font-size="{}" {}>"##,
        // TODO Plumb through default font size?
        spans[0].size.unwrap_or(30),
        match spans[0].font {
            Font::DejaVu => "font-family=\"DejaVu Sans\"",
            Font::Roboto => "font-family=\"Roboto\"",
            Font::RobotoBold => "font-family=\"Roboto\" font-weight=\"bold\"",
        }
    );

    let mut contents = String::new();
    for span in spans {
        write!(
            &mut contents,
            r##"<tspan fill="{}">{}</tspan>"##,
            span.fg_color.to_hex(),
            span.text
        )
        .unwrap();
    }
    write!(&mut svg, "{}</text></svg>", contents).unwrap();

    //println!("- Rendering: {}", contents);

    let mut opts = usvg::Options::default();
    // TODO Bundle better
    opts.font_directories
        .push("/home/dabreegster/abstreet/ezgui/src/assets".to_string());
    let svg_tree = match usvg::Tree::from_str(&svg, &opts) {
        Ok(t) => t,
        Err(err) => panic!("render_text({}): {}", contents, err),
    };
    let mut batch = GeomBatch::new();
    match crate::svg::add_svg_inner(&mut batch, svg_tree) {
        Ok(_) => batch,
        Err(err) => panic!("render_text({}): {}", contents, err),
    }
}
