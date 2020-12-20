use abstutil::prettyprint_usize;
use geom::Polygon;

use crate::{Btn, Color, EventCtx, GeomBatch, Key, Line, Panel, Text, TextExt, Widget};

const ROWS: usize = 8;

pub struct Table<A, T, F> {
    data: Vec<T>,
    label_per_row: Box<dyn Fn(&T) -> String>,
    columns: Vec<Column<A, T>>,
    filter: Filter<A, T, F>,

    sort_by: String,
    descending: bool,
    skip: usize,
}

pub enum Col<T> {
    Static,
    Sortable(Box<dyn Fn(&mut Vec<&T>)>),
}

struct Column<A, T> {
    name: String,
    render: Box<dyn Fn(&EventCtx, &A, &T) -> GeomBatch>,
    col: Col<T>,
}

pub struct Filter<A, T, F> {
    pub state: F,
    pub to_controls: Box<dyn Fn(&mut EventCtx, &A, &F) -> Widget>,
    pub from_controls: Box<dyn Fn(&Panel) -> F>,
    pub apply: Box<dyn Fn(&F, &T) -> bool>,
}

impl<A, T, F> Table<A, T, F> {
    pub fn new(
        data: Vec<T>,
        label_per_row: Box<dyn Fn(&T) -> String>,
        default_sort_by: &str,
        filter: Filter<A, T, F>,
    ) -> Table<A, T, F> {
        Table {
            data,
            label_per_row,
            columns: Vec::new(),
            filter,

            sort_by: default_sort_by.to_string(),
            descending: true,
            skip: 0,
        }
    }

    pub fn column(
        &mut self,
        name: &str,
        render: Box<dyn Fn(&EventCtx, &A, &T) -> GeomBatch>,
        col: Col<T>,
    ) {
        self.columns.push(Column {
            name: name.to_string(),
            render,
            col,
        });
    }

    pub fn render(&self, ctx: &mut EventCtx, app: &A) -> Widget {
        let mut data: Vec<&T> = Vec::new();

        // Filter
        for row in &self.data {
            if (self.filter.apply)(&self.filter.state, row) {
                data.push(row);
            }
        }
        let num_filtered = data.len();

        // Sort
        for col in &self.columns {
            if col.name == self.sort_by {
                if let Col::Sortable(ref sorter) = col.col {
                    (sorter)(&mut data);
                    break;
                }
                // TODO Error handling
            }
        }
        if self.descending {
            data.reverse();
        }

        // Render the headers
        let headers = self
            .columns
            .iter()
            .map(|col| {
                if self.sort_by == col.name {
                    Btn::text_bg2(format!(
                        "{} {}",
                        col.name,
                        if self.descending { "↓" } else { "↑" }
                    ))
                    .build(ctx, &col.name, None)
                } else if let Col::Sortable(_) = col.col {
                    Btn::text_bg2(&col.name).build_def(ctx, None)
                } else {
                    Line(&col.name).draw(ctx).centered_vert()
                }
            })
            .collect();

        // Render data
        let mut rows = Vec::new();
        for row in data.into_iter().skip(self.skip).take(ROWS) {
            rows.push((
                (self.label_per_row)(row),
                self.columns
                    .iter()
                    .map(|col| (col.render)(ctx, app, row))
                    .collect(),
            ));
        }

        // Put together the UI
        Widget::col(vec![
            (self.filter.to_controls)(ctx, app, &self.filter.state),
            make_table(ctx, headers, rows, 0.88 * ctx.canvas.window_width),
            make_pagination(ctx, num_filtered, self.skip),
        ])
    }

    // Recalculate if true
    pub fn clicked(&mut self, action: &str) -> bool {
        if action == "previous" {
            self.skip -= ROWS;
            return true;
        }
        if action == "next" {
            self.skip += ROWS;
            return true;
        }
        for col in &self.columns {
            if col.name == action {
                self.skip = 0;
                if self.sort_by == action {
                    self.descending = !self.descending;
                } else {
                    self.sort_by = action.to_string();
                    self.descending = true;
                }
                return true;
            }
        }
        false
    }

    pub fn panel_changed(&mut self, panel: &Panel) {
        self.filter.state = (self.filter.from_controls)(panel);
        self.skip = 0;
    }
}

impl<A, T> Filter<A, T, ()> {
    pub fn empty() -> Filter<A, T, ()> {
        Filter {
            state: (),
            to_controls: Box::new(|_, _, _| Widget::nothing()),
            from_controls: Box::new(|_| ()),
            apply: Box::new(|_, _| true),
        }
    }
}

// Simpler wrappers than column(). The more generic case exists to allow for icons and non-text
// things.
impl<A, T: 'static, F> Table<A, T, F> {
    pub fn static_col(&mut self, name: &str, to_str: Box<dyn Fn(&T) -> String>) {
        self.column(
            name,
            Box::new(move |ctx, _, x| Text::from(Line((to_str)(x))).render(ctx)),
            Col::Static,
        );
    }
}

fn make_pagination(ctx: &mut EventCtx, total: usize, skip: usize) -> Widget {
    Widget::row(vec![
        if skip > 0 {
            Btn::plaintext("<").build(ctx, "previous", Key::LeftArrow)
        } else {
            Btn::plaintext("<").inactive(ctx)
        },
        format!(
            "{}-{} of {}",
            if total > 0 {
                prettyprint_usize(skip + 1)
            } else {
                "0".to_string()
            },
            prettyprint_usize((skip + 1 + ROWS).min(total)),
            prettyprint_usize(total)
        )
        .draw_text(ctx)
        .centered_vert(),
        if skip + 1 + ROWS < total {
            Btn::plaintext(">").build(ctx, "next", Key::RightArrow)
        } else {
            Btn::plaintext(">").inactive(ctx)
        },
    ])
}

fn make_table(
    ctx: &mut EventCtx,
    headers: Vec<Widget>,
    rows: Vec<(String, Vec<GeomBatch>)>,
    total_width: f64,
) -> Widget {
    let total_width = total_width;
    let mut width_per_col: Vec<f64> = headers.iter().map(|w| w.get_width_for_forcing()).collect();
    for (_, row) in &rows {
        for (col, width) in row.iter().zip(width_per_col.iter_mut()) {
            *width = width.max(col.get_dims().width);
        }
    }
    let extra_margin = ((total_width - width_per_col.clone().into_iter().sum::<f64>())
        / (width_per_col.len() - 1) as f64)
        .max(0.0);

    let mut col = vec![Widget::custom_row(
        headers
            .into_iter()
            .enumerate()
            .map(|(idx, w)| {
                let margin = extra_margin + width_per_col[idx] - w.get_width_for_forcing();
                if idx == width_per_col.len() - 1 {
                    w.margin_right((margin - extra_margin) as usize)
                } else {
                    w.margin_right(margin as usize)
                }
            })
            .collect(),
    )];

    // TODO Maybe can do this now simpler with to_geom
    for (label, row) in rows {
        let mut batch = GeomBatch::new();
        batch.autocrop_dims = false;
        let mut x1 = 0.0;
        for (col, width) in row.into_iter().zip(width_per_col.iter()) {
            batch.append(col.translate(x1, 0.0));
            x1 += *width + extra_margin;
        }

        let rect = Polygon::rectangle(total_width, batch.get_dims().height);
        let mut hovered = GeomBatch::new();
        hovered.push(Color::hex("#7C7C7C"), rect.clone());
        hovered.append(batch.clone());

        col.push(
            Btn::custom(batch, hovered, rect, None)
                .tooltip(Text::new())
                .build(ctx, label, None),
        );
    }

    Widget::custom_col(col)
}
