use crate::app::App;
use crate::layer::Layers;
use ezgui::{
    Btn, Color, Composite, EventCtx, Histogram, HorizontalAlignment, Line, Text, VerticalAlignment,
    Widget,
};

pub fn trips_histogram(ctx: &mut EventCtx, app: &App) -> Layers {
    if app.has_prebaked().is_none() {
        return Layers::Inactive;
    }

    let now = app.primary.sim.time();
    Layers::TripsHistogram(
        now,
        Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    {
                        let mut txt = Text::from(Line("Are trips "));
                        txt.append(Line("faster").fg(Color::GREEN));
                        txt.append(Line(", "));
                        txt.append(Line("slower").fg(Color::RED));
                        txt.append(Line(", or "));
                        txt.append(Line("the same").fg(Color::YELLOW));
                        txt.append(Line("?"));
                        txt.draw(ctx)
                    }
                    .margin(10),
                    Btn::text_fg("X").build_def(ctx, None).align_right(),
                ]),
                Histogram::new(
                    app.primary
                        .sim
                        .get_analytics()
                        .trip_time_deltas(now, app.prebaked()),
                    ctx,
                ),
            ])
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx),
    )
}
