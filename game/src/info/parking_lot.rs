use crate::app::App;
use crate::info::{header_btns, make_tabs, Details, Tab};
use abstutil::prettyprint_usize;
use ezgui::{EventCtx, Line, LinePlot, PlotOptions, Series, TextExt, Widget};
use map_model::ParkingLotID;
use std::collections::HashSet;

pub fn info(ctx: &mut EventCtx, app: &App, details: &mut Details, id: ParkingLotID) -> Vec<Widget> {
    let mut rows = header(ctx, details, id, Tab::ParkingLot(id));
    let pl = app.primary.map.get_pl(id);
    let capacity = pl.spots.len();

    rows.push(
        format!(
            "{} / {} spots available",
            prettyprint_usize(app.primary.sim.get_free_lot_spots(pl.id).len()),
            prettyprint_usize(capacity)
        )
        .draw_text(ctx),
    );

    let mut series = vec![Series {
        label: format!("After \"{}\"", app.primary.map.get_edits().edits_name),
        color: app.cs.after_changes,
        pts: app.primary.sim.get_analytics().parking_lot_availability(
            app.primary.sim.time(),
            pl.id,
            capacity,
        ),
    }];
    if app.has_prebaked().is_some() {
        series.push(Series {
            label: format!("Before \"{}\"", app.primary.map.get_edits().edits_name),
            color: app.cs.before_changes.alpha(0.5),
            pts: app.prebaked().parking_lot_availability(
                app.primary.sim.get_end_of_day(),
                pl.id,
                capacity,
            ),
        });
    }
    rows.push("Parking spots available".draw_text(ctx));
    rows.push(LinePlot::new(
        ctx,
        series,
        PlotOptions {
            filterable: false,
            max_x: None,
            max_y: Some(capacity),
            disabled: HashSet::new(),
            downsample_pts: None,
        },
    ));

    rows
}

fn header(ctx: &EventCtx, details: &mut Details, id: ParkingLotID, tab: Tab) -> Vec<Widget> {
    vec![
        Widget::row(vec![
            Line(id.to_string()).small_heading().draw(ctx),
            header_btns(ctx),
        ]),
        make_tabs(
            ctx,
            &mut details.hyperlinks,
            tab,
            vec![("Info", Tab::ParkingLot(id))],
        ),
    ]
}
