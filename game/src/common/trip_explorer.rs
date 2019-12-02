use crate::common::ColorLegend;
use crate::game::{State, Transition};
use crate::helpers::rotating_color_map;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{hotkey, Drawable, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu, Text};
use geom::{Circle, Distance};
use sim::{TripEnd, TripID, TripStart};

pub struct TripExplorer {
    menu: ModalMenu,
    unzoomed: Drawable,
    zoomed: Drawable,
    legend: ColorLegend,
}

impl TripExplorer {
    pub fn new(trip: TripID, ctx: &mut EventCtx, ui: &UI) -> TripExplorer {
        let phases = ui
            .primary
            .sim
            .get_analytics()
            .get_trip_phases(trip, &ui.primary.map);
        // TODO Hack because ColorLegend only takes &str
        let mut rows = Vec::new();
        for (idx, p) in phases.iter().enumerate() {
            rows.push((
                p.describe(ui.primary.sim.time()),
                rotating_color_map(idx + 1),
            ));
        }
        let mut unzoomed = GeomBatch::new();
        let mut zoomed = GeomBatch::new();
        for (p, (_, color)) in phases.iter().zip(rows.iter()) {
            if let Some((dist, ref path)) = p.path {
                if let Some(t) = path.trace(&ui.primary.map, dist, None) {
                    unzoomed.push(*color, t.make_polygons(Distance::meters(10.0)));
                    zoomed.push(*color, t.make_polygons(Distance::meters(1.0)));
                }
            }
        }

        // Handle endpoints
        let status = ui.primary.sim.trip_status(trip);
        let start_color = rotating_color_map(0);
        match status.start {
            TripStart::Bldg(b) => {
                let bldg = ui.primary.map.get_b(b);
                rows.insert(0, (format!("start at {}", bldg.get_name()), start_color));
                unzoomed.push(start_color, bldg.polygon.clone());
                zoomed.push(start_color, bldg.polygon.clone());
            }
            TripStart::Border(i) => {
                let i = ui.primary.map.get_i(i);
                rows.insert(0, (format!("enter map via {}", i.id), start_color));
                unzoomed.push(start_color, i.polygon.clone());
                zoomed.push(start_color, i.polygon.clone());
            }
        };

        // Is the trip ongoing?
        if let Some(pt) = ui
            .primary
            .sim
            .get_canonical_pt_per_trip(trip, &ui.primary.map)
            .ok()
        {
            let color = rotating_color_map(rows.len());
            unzoomed.push(color, Circle::new(pt, Distance::meters(10.0)).to_polygon());
            zoomed.push(
                color.alpha(0.7),
                Circle::new(pt, Distance::meters(5.0)).to_polygon(),
            );
            rows.push((format!("currently here"), color));
        }

        let end_color = rotating_color_map(rows.len());
        match status.end {
            TripEnd::Bldg(b) => {
                let bldg = ui.primary.map.get_b(b);
                rows.push((format!("end at {}", bldg.get_name()), end_color));
                unzoomed.push(end_color, bldg.polygon.clone());
                zoomed.push(end_color, bldg.polygon.clone());
            }
            TripEnd::Border(i) => {
                let i = ui.primary.map.get_i(i);
                rows.push((format!("leave map via {}", i.id), end_color));
                unzoomed.push(end_color, i.polygon.clone());
                zoomed.push(end_color, i.polygon.clone());
            }
            // TODO TripExplorer is pretty useless for buses; maybe jump to BusRouteExplorer or
            // something instead
            TripEnd::ServeBusRoute(br) => {
                rows.push((
                    format!("serve route {} forever", ui.primary.map.get_br(br).name),
                    end_color,
                ));
            }
        };

        let legend = ColorLegend::new(
            Text::prompt(&trip.to_string()),
            rows.iter()
                .map(|(label, color)| (label.as_str(), *color))
                .collect(),
        );

        TripExplorer {
            menu: ModalMenu::new(trip.to_string(), vec![(hotkey(Key::Escape), "quit")], ctx),
            legend,
            unzoomed: unzoomed.upload(ctx),
            zoomed: zoomed.upload(ctx),
        }
    }
}

impl State for TripExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        self.menu.event(ctx);
        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.legend.draw(g);
        self.menu.draw(g);
    }
}
