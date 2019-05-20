use crate::helpers::rotating_color;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{GPSBounds, Polygon};
use popdat::PopDat;

pub struct DataVisualizer {
    menu: ModalMenu,
    popdat: PopDat,
    tracts: Vec<Tract>,

    // TODO Urgh. 0, 1, or 2.
    current_dataset: usize,
}

struct Tract {
    name: String,
    polygon: Polygon,
    color: Color,
}

impl DataVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> DataVisualizer {
        let mut timer = Timer::new("initialize popdat");
        let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat", &mut timer)
            .expect("Couldn't load popdat");

        DataVisualizer {
            menu: ModalMenu::new(
                "Data Visualizer",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::Num1), "household vehicles"),
                    (Some(Key::Num2), "commute times"),
                    (Some(Key::Num3), "commute modes"),
                ],
                ctx,
            ),
            tracts: clip(&popdat, &ui.primary.map.get_gps_bounds()),
            popdat,
            current_dataset: 0,
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> bool {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        // TODO Remember which dataset we're showing and don't allow reseting to the same.
        if self.menu.action("quit") {
            return true;
        } else if self.current_dataset != 0 && self.menu.action("household vehicles") {
            self.current_dataset = 0;
        } else if self.current_dataset != 1 && self.menu.action("commute times") {
            self.current_dataset = 1;
        } else if self.current_dataset != 2 && self.menu.action("commute modes") {
            self.current_dataset = 2;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        for tract in &self.tracts {
            g.draw_polygon(tract.color, &tract.polygon);
        }

        self.menu.draw(g);
    }
}

fn clip(popdat: &PopDat, bounds: &GPSBounds) -> Vec<Tract> {
    // TODO Partial clipping could be neat, except it'd be confusing to interpret totals.
    let mut results = Vec::new();
    for (name, tract) in &popdat.tracts {
        if let Some(pts) = bounds.try_convert(&tract.pts) {
            // TODO We should actually make sure the polygon is completely contained within the
            // map's boundary.
            results.push(Tract {
                name: name.clone(),
                polygon: Polygon::new(&pts),
                color: rotating_color(results.len()),
            });
        }
    }
    println!(
        "Clipped {} tracts from {}",
        results.len(),
        popdat.tracts.len()
    );
    results
}
