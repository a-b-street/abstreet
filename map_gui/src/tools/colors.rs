use std::collections::HashMap;

use abstutil::Counter;
use geom::{Circle, Distance};
use map_model::{BuildingID, IntersectionID, LaneID, Map, ParkingLotID, RoadID, TransitStopID};
use widgetry::mapspace::{ToggleZoomed, ToggleZoomedBuilder};
use widgetry::tools::{ColorLegend, ColorScale};
use widgetry::{Color, EventCtx, GeomBatch, Widget};

use crate::AppLike;

// TODO Tooltips would almost be nice, for cases like pedestrian crowding
pub struct ColorDiscrete<'a> {
    map: &'a Map,
    // pub so callers can add stuff in before building
    pub draw: ToggleZoomedBuilder,
    // Store both, so we can build the legend in the original order later
    pub categories: Vec<(String, Color)>,
    colors: HashMap<String, Color>,
}

impl<'a> ColorDiscrete<'a> {
    pub fn new<I: Into<String>>(
        app: &'a dyn AppLike,
        categories: Vec<(I, Color)>,
    ) -> ColorDiscrete<'a> {
        let mut draw = ToggleZoomed::builder();
        draw.unzoomed.push(
            app.cs().fade_map_dark,
            app.map().get_boundary_polygon().clone(),
        );
        let categories: Vec<(String, Color)> =
            categories.into_iter().map(|(k, v)| (k.into(), v)).collect();
        ColorDiscrete {
            map: app.map(),
            draw,
            colors: categories.iter().cloned().collect(),
            categories,
        }
    }

    pub fn no_fading<I: Into<String>>(
        app: &'a dyn AppLike,
        categories: Vec<(I, Color)>,
    ) -> ColorDiscrete<'a> {
        let mut c = ColorDiscrete::new(app, categories);
        c.draw.unzoomed = GeomBatch::new();
        c
    }

    pub fn add_l<I: AsRef<str>>(&mut self, l: LaneID, category: I) {
        let color = self.colors[category.as_ref()];
        self.draw
            .unzoomed
            .push(color, self.map.get_parent(l).get_thick_polygon());
        let lane = self.map.get_l(l);
        self.draw
            .zoomed
            .push(color.alpha(0.4), lane.get_thick_polygon());
    }

    pub fn add_r<I: AsRef<str>>(&mut self, r: RoadID, category: I) {
        let color = self.colors[category.as_ref()];
        self.draw
            .unzoomed
            .push(color, self.map.get_r(r).get_thick_polygon());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_r(r).get_thick_polygon());
    }

    pub fn add_i<I: AsRef<str>>(&mut self, i: IntersectionID, category: I) {
        let color = self.colors[category.as_ref()];
        self.draw
            .unzoomed
            .push(color, self.map.get_i(i).polygon.clone());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_i(i).polygon.clone());
    }

    pub fn add_b<I: AsRef<str>>(&mut self, b: BuildingID, category: I) {
        let color = self.colors[category.as_ref()];
        self.draw
            .unzoomed
            .push(color, self.map.get_b(b).polygon.clone());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_b(b).polygon.clone());
    }

    pub fn add_ts<I: AsRef<str>>(&mut self, ts: TransitStopID, category: I) {
        let color = self.colors[category.as_ref()];
        let pt = self.map.get_ts(ts).sidewalk_pos.pt(self.map);
        self.draw.zoomed.push(
            color.alpha(0.4),
            Circle::new(pt, Distance::meters(5.0)).to_polygon(),
        );
        self.draw
            .unzoomed
            .push(color, Circle::new(pt, Distance::meters(15.0)).to_polygon());
    }

    pub fn build(self, ctx: &EventCtx) -> (ToggleZoomed, Widget) {
        let legend = self
            .categories
            .into_iter()
            .map(|(name, color)| ColorLegend::row(ctx, color, name))
            .collect();
        (self.draw.build(ctx), Widget::col(legend))
    }
}

// TODO Bad name
pub struct ColorNetwork<'a> {
    map: &'a Map,
    pub draw: ToggleZoomedBuilder,
}

impl<'a> ColorNetwork<'a> {
    pub fn new(app: &'a dyn AppLike) -> ColorNetwork {
        let mut draw = ToggleZoomed::builder();
        draw.unzoomed.push(
            app.cs().fade_map_dark,
            app.map().get_boundary_polygon().clone(),
        );
        ColorNetwork {
            map: app.map(),
            draw,
        }
    }

    pub fn no_fading(app: &'a dyn AppLike) -> ColorNetwork {
        ColorNetwork {
            map: app.map(),
            draw: ToggleZoomed::builder(),
        }
    }

    pub fn add_l(&mut self, l: LaneID, color: Color) {
        self.draw
            .unzoomed
            .push(color, self.map.get_parent(l).get_thick_polygon());
        let lane = self.map.get_l(l);
        self.draw
            .zoomed
            .push(color.alpha(0.4), lane.get_thick_polygon());
    }

    pub fn add_r(&mut self, r: RoadID, color: Color) {
        self.draw
            .unzoomed
            .push(color, self.map.get_r(r).get_thick_polygon());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_r(r).get_thick_polygon());
    }

    pub fn add_i(&mut self, i: IntersectionID, color: Color) {
        self.draw
            .unzoomed
            .push(color, self.map.get_i(i).polygon.clone());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_i(i).polygon.clone());
    }

    pub fn add_b(&mut self, b: BuildingID, color: Color) {
        self.draw
            .unzoomed
            .push(color, self.map.get_b(b).polygon.clone());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_b(b).polygon.clone());
    }

    pub fn add_pl(&mut self, pl: ParkingLotID, color: Color) {
        self.draw
            .unzoomed
            .push(color, self.map.get_pl(pl).polygon.clone());
        self.draw
            .zoomed
            .push(color.alpha(0.4), self.map.get_pl(pl).polygon.clone());
    }

    // Order the roads by count, then interpolate a color based on position in that ordering.
    pub fn ranked_roads(&mut self, counter: Counter<RoadID>, scale: &ColorScale) {
        let roads = counter.sorted_asc();
        let len = roads.len() as f64;
        for (idx, list) in roads.into_iter().enumerate() {
            let color = scale.eval((idx as f64) / len);
            for r in list {
                self.add_r(r, color);
            }
        }
    }
    pub fn ranked_intersections(&mut self, counter: Counter<IntersectionID>, scale: &ColorScale) {
        let intersections = counter.sorted_asc();
        let len = intersections.len() as f64;
        for (idx, list) in intersections.into_iter().enumerate() {
            let color = scale.eval((idx as f64) / len);
            for i in list {
                self.add_i(i, color);
            }
        }
    }

    // Interpolate a color for each road based on the max count.
    pub fn pct_roads(&mut self, counter: Counter<RoadID>, scale: &ColorScale) {
        let max = counter.max() as f64;
        for (r, cnt) in counter.consume() {
            self.add_r(r, scale.eval((cnt as f64) / max));
        }
    }
    // Interpolate a color for each intersection based on the max count.
    pub fn pct_intersections(&mut self, counter: Counter<IntersectionID>, scale: &ColorScale) {
        let max = counter.max() as f64;
        for (i, cnt) in counter.consume() {
            self.add_i(i, scale.eval((cnt as f64) / max));
        }
    }

    pub fn build(self, ctx: &EventCtx) -> ToggleZoomed {
        self.draw.build(ctx)
    }
}
