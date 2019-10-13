use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::{rotating_color_total, ID};
use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use ezgui::{
    hotkey, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment,
};
use geom::{Distance, Polygon, Pt2D};
use popdat::{Estimate, PopDat};
use std::collections::BTreeMap;

pub struct DataVisualizer {
    menu: ModalMenu,
    popdat: PopDat,
    tracts: BTreeMap<String, Tract>,

    // Table if false
    show_bars: bool,
    // TODO Urgh. 0, 1, or 2.
    current_dataset: usize,
    current_tract: Option<String>,
}

struct Tract {
    polygon: Polygon,
    color: Color,

    num_bldgs: usize,
    num_parking_spots: usize,
    total_owned_cars: usize,
}

impl DataVisualizer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> DataVisualizer {
        let (popdat, tracts) = ctx.loading_screen("initialize popdat", |_, mut timer| {
            let popdat: PopDat = abstutil::read_binary("../data/shapes/popdat.bin", &mut timer)
                .expect("Couldn't load popdat.bin");
            let tracts = clip_tracts(&popdat, ui, &mut timer);
            (popdat, tracts)
        });

        DataVisualizer {
            menu: ModalMenu::new(
                "Data Visualizer",
                vec![
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::Space), "toggle table/bar chart"),
                    ],
                    vec![
                        (hotkey(Key::Num1), "household vehicles"),
                        (hotkey(Key::Num2), "commute times"),
                        (hotkey(Key::Num3), "commute modes"),
                    ],
                ],
                ctx,
            ),
            tracts,
            popdat,
            show_bars: false,
            current_dataset: 0,
            current_tract: None,
        }
    }
}
impl State for DataVisualizer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        {
            let mut txt = Text::new();
            if let Some(ref name) = self.current_tract {
                txt.add_appended(vec![
                    Line("Census "),
                    Line(name).fg(ui.cs.get("OSD name color")),
                ]);
                let tract = &self.tracts[name];
                txt.add(Line(format!(
                    "{} buildings",
                    prettyprint_usize(tract.num_bldgs)
                )));
                txt.add(Line(format!(
                    "{} parking spots ",
                    prettyprint_usize(tract.num_parking_spots)
                )));
                txt.add(Line(format!(
                    "{} total owned cars",
                    prettyprint_usize(tract.total_owned_cars)
                )));
            }
            self.menu.set_info(ctx, txt);
        }
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);

        // TODO Remember which dataset we're showing and don't allow reseting to the same.
        if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.current_dataset != 0 && self.menu.action("household vehicles") {
            self.current_dataset = 0;
        } else if self.current_dataset != 1 && self.menu.action("commute times") {
            self.current_dataset = 1;
        } else if self.current_dataset != 2 && self.menu.action("commute modes") {
            self.current_dataset = 2;
        } else if self.menu.action("toggle table/bar chart") {
            self.show_bars = !self.show_bars;
        }

        if ctx.redo_mouseover() {
            self.current_tract = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for (name, tract) in &self.tracts {
                    if tract.polygon.contains_pt(pt) {
                        self.current_tract = Some(name.clone());
                        break;
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        for (name, tract) in &self.tracts {
            let color = if Some(name.clone()) == self.current_tract {
                ui.cs.get("selected")
            } else {
                tract.color
            };
            g.draw_polygon(color, &tract.polygon);
        }

        self.menu.draw(g);
        if let Some(ref name) = self.current_tract {
            let mut osd = Text::new();
            osd.add_appended(vec![
                Line("Census "),
                Line(name).fg(ui.cs.get("OSD name color")),
            ]);
            CommonState::draw_custom_osd(g, osd);
        } else {
            CommonState::draw_osd(g, ui, &None);
        }

        if let Some(ref name) = self.current_tract {
            let tract = &self.popdat.tracts[name];
            let kv = if self.current_dataset == 0 {
                &tract.household_vehicles
            } else if self.current_dataset == 1 {
                &tract.commute_times
            } else if self.current_dataset == 2 {
                &tract.commute_modes
            } else {
                unreachable!()
            };

            if self.show_bars {
                bar_chart(g, kv);
            } else {
                let mut txt = Text::new();
                for (k, v) in kv {
                    txt.add_appended(vec![
                        Line(k).fg(Color::RED),
                        Line(" = "),
                        Line(v.to_string()).fg(Color::CYAN),
                    ]);
                }
                g.draw_blocking_text(&txt, (HorizontalAlignment::Left, VerticalAlignment::Top));
            }
        }
    }
}

fn clip_tracts(popdat: &PopDat, ui: &UI, timer: &mut Timer) -> BTreeMap<String, Tract> {
    // TODO Partial clipping could be neat, except it'd be confusing to interpret totals.
    let mut results = BTreeMap::new();
    timer.start_iter("clip tracts", popdat.tracts.len());
    for (name, tract) in &popdat.tracts {
        timer.next();
        if let Some(pts) = ui.primary.map.get_gps_bounds().try_convert(&tract.pts) {
            // TODO We should actually make sure the polygon is completely contained within the
            // map's boundary.
            let polygon = Polygon::new(&pts);

            // TODO Don't just use the center...
            let mut num_bldgs = 0;
            let mut num_parking_spots = 0;
            for id in ui
                .primary
                .draw_map
                .get_matching_objects(polygon.get_bounds())
            {
                match id {
                    ID::Building(b) => {
                        if polygon.contains_pt(ui.primary.map.get_b(b).polygon.center()) {
                            num_bldgs += 1;
                        }
                    }
                    ID::Lane(l) => {
                        let lane = ui.primary.map.get_l(l);
                        if lane.is_parking() && polygon.contains_pt(lane.lane_center_pts.middle()) {
                            num_parking_spots += lane.number_parking_spots();
                        }
                    }
                    _ => {}
                }
            }

            results.insert(
                name.clone(),
                Tract {
                    polygon,
                    // Update it after we know the total number of matching tracts.
                    color: Color::WHITE,
                    num_bldgs,
                    num_parking_spots,
                    total_owned_cars: tract.total_owned_cars(),
                },
            );
        }
    }
    let len = results.len();
    for (idx, tract) in results.values_mut().enumerate() {
        tract.color = rotating_color_total(idx, len);
    }
    println!(
        "Clipped {} tracts from {}",
        results.len(),
        popdat.tracts.len()
    );
    results
}

fn bar_chart(g: &mut GfxCtx, data: &BTreeMap<String, Estimate>) {
    let mut max = 0;
    let mut sum = 0;
    for (name, est) in data {
        if name == "Total:" {
            continue;
        }
        max = max.max(est.value);
        sum += est.value;
    }

    let mut labels = Text::with_bg_color(None);
    for (name, est) in data {
        if name == "Total:" {
            continue;
        }
        labels.add_appended(vec![
            Line(format!("{} (", name)).size(40),
            Line(format!(
                "{}%",
                ((est.value as f64) / (sum as f64) * 100.0) as usize
            ))
            .fg(Color::RED),
            Line(")"),
        ]);
    }
    let (txt_width, total_height) = g.text_dims(&labels);
    let line_height = total_height / ((data.len() as f64) - 1.0);
    labels.add(Line(format!("{} samples", prettyprint_usize(sum))).size(40));

    // This is, uh, pixels. :P
    let max_bar_width = 300.0;

    g.fork_screenspace();
    g.draw_polygon(
        Color::grey(0.3),
        &Polygon::rectangle_topleft(
            Pt2D::new(0.0, 0.0),
            Distance::meters(txt_width + 1.2 * max_bar_width),
            Distance::meters(total_height + line_height),
        ),
    );
    g.draw_blocking_text(&labels, (HorizontalAlignment::Left, VerticalAlignment::Top));
    // draw_blocking_text undoes this! Oops.
    g.fork_screenspace();

    for (idx, (name, est)) in data.iter().enumerate() {
        if name == "Total:" {
            continue;
        }
        let this_width = max_bar_width * ((est.value as f64) / (max as f64));
        g.draw_polygon(
            rotating_color_total(idx, data.len() - 1),
            &Polygon::rectangle_topleft(
                Pt2D::new(txt_width, (0.1 + (idx as f64)) * line_height),
                Distance::meters(this_width),
                Distance::meters(0.8 * line_height),
            ),
        );

        // Error bars!
        // TODO Little cap on both sides
        let half_moe_width = max_bar_width * (est.moe as f64) / (max as f64) / 2.0;
        g.draw_polygon(
            Color::BLACK,
            &Polygon::rectangle_topleft(
                Pt2D::new(
                    txt_width + this_width - half_moe_width,
                    (0.4 + (idx as f64)) * line_height,
                ),
                2.0 * Distance::meters(half_moe_width),
                0.2 * Distance::meters(line_height),
            ),
        );
    }

    g.unfork();
}
