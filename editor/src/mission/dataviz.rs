use crate::common::CommonState;
use crate::helpers::rotating_color;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu, Text};
use geom::{GPSBounds, Polygon};
use popdat::PopDat;

pub struct DataVisualizer {
    menu: ModalMenu,
    popdat: PopDat,
    tracts: Vec<Tract>,

    // TODO Urgh. 0, 1, or 2.
    current_dataset: usize,
    current_tract: Option<String>,
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
            current_tract: None,
        }
    }

    // Returns true if the we're done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> bool {
        let mut txt = Text::prompt("Data Visualizer");
        if let Some(ref name) = self.current_tract {
            txt.add_line("Census ".to_string());
            txt.append(name.clone(), Some(ui.cs.get("OSD name color")));
        }
        self.menu.handle_event(ctx, Some(txt));
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

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.current_tract = None;
                for tract in &self.tracts {
                    if tract.polygon.contains_pt(pt) {
                        self.current_tract = Some(tract.name.clone());
                        break;
                    }
                }
            }
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        for tract in &self.tracts {
            let color = if Some(tract.name.clone()) == self.current_tract {
                ui.cs.get("selected")
            } else {
                tract.color
            };
            g.draw_polygon(color, &tract.polygon);
        }

        self.menu.draw(g);
        if let Some(ref name) = self.current_tract {
            let mut osd = Text::new();
            osd.add_line("Census ".to_string());
            osd.append(name.clone(), Some(ui.cs.get("OSD name color")));
            CommonState::draw_custom_osd(g, osd);
        } else {
            CommonState::draw_osd(g, ui, None);
        }
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
