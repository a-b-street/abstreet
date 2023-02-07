use geom::{Polygon, Pt2D};
use map_model::{BuildingID, IntersectionID};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::ChooseSomething;
use widgetry::{
    Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct AreaSpawner {
    areas: Vec<Area>,
    panel: Panel,
    world: World<Obj>,
    mode: Mode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Obj(usize);
impl ObjectID for Obj {}

enum Mode {
    Neutral,
    DrawingArea(SelectRectangle),
    PickingDestination { source: usize },
}

impl AreaSpawner {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        Box::new(AreaSpawner {
            areas: Vec::new(),
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Specify traffic patterns")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("Draw new area")
                    .hotkey(Key::A)
                    .build_def(ctx),
                "".text_widget(ctx).named("instructions"),
            ]))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx),
            world: World::new(),
            mode: Mode::Neutral,
        })
    }

    fn rebuild_world(&mut self, ctx: &mut EventCtx) {
        let mut world = World::new();
        let picking_destination = match self.mode {
            Mode::PickingDestination { source } => Some(source),
            _ => None,
        };

        for (idx, area) in self.areas.iter().enumerate() {
            if picking_destination == Some(idx) {
                world
                    .add(Obj(idx))
                    .hitbox(area.polygon.clone())
                    .draw_color(Color::RED.alpha(0.5))
                    .build(ctx);
            } else {
                world
                    .add(Obj(idx))
                    .hitbox(area.polygon.clone())
                    .draw_color(Color::BLUE.alpha(0.5))
                    .hover_alpha(0.8)
                    .clickable()
                    .build(ctx);
            }
        }
        world.initialize_hover(ctx);
        self.world = world;
    }
}

impl State<App> for AreaSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.mode {
            Mode::Neutral => {
                if let Outcome::Clicked(x) = self.panel.event(ctx) {
                    match x.as_ref() {
                        "close" => {
                            return Transition::Pop;
                        }
                        "Draw new area" => {
                            self.mode = Mode::DrawingArea(SelectRectangle::new(ctx));
                            let label = "Click and drag to select an area".text_widget(ctx);
                            self.panel.replace(ctx, "instructions", label);
                        }
                        _ => unreachable!(),
                    }
                }

                if let WorldOutcome::ClickedObject(Obj(idx)) = self.world.event(ctx) {
                    let area = &self.areas[idx];
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        format!(
                            "This area has {} buildings and {} borders",
                            area.buildings.len(),
                            area.borders.len()
                        ),
                        vec![
                            Choice::string("spawn traffic from here"),
                            Choice::string("delete"),
                        ],
                        Box::new(move |resp, _, _| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ModifyState(Box::new(move |state, ctx, _| {
                                    let state = state.downcast_mut::<AreaSpawner>().unwrap();
                                    if resp == "delete" {
                                        state.areas.remove(idx);
                                        state.rebuild_world(ctx);
                                    } else if resp == "spawn traffic from here" {
                                        state.mode = Mode::PickingDestination { source: idx };
                                        state.rebuild_world(ctx);
                                        let label = "Choose where traffic will go".text_widget(ctx);
                                        state.panel.replace(ctx, "instructions", label);
                                    }
                                })),
                            ])
                        }),
                    ));
                }
            }
            Mode::DrawingArea(ref mut select) => {
                if select.event(ctx) {
                    if let Some(polygon) = select.rect.take() {
                        self.areas.push(Area::new(app, polygon));
                        self.rebuild_world(ctx);
                    }
                    self.mode = Mode::Neutral;
                    let label = "".text_widget(ctx);
                    self.panel.replace(ctx, "instructions", label);
                }
            }
            Mode::PickingDestination { .. } => {
                if let WorldOutcome::ClickedObject(Obj(_destination)) = self.world.event(ctx) {
                    // TODO Enter a new state to specify the traffic params
                    self.mode = Mode::Neutral;
                    self.rebuild_world(ctx);
                    let label = "".text_widget(ctx);
                    self.panel.replace(ctx, "instructions", label);
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.world.draw(g);
        if let Mode::DrawingArea(ref select) = self.mode {
            select.draw(g);
        }
    }
}

struct SelectRectangle {
    pt1: Option<Pt2D>,
    rect: Option<Polygon>,
    preview: Drawable,
}

impl SelectRectangle {
    fn new(ctx: &mut EventCtx) -> SelectRectangle {
        SelectRectangle {
            pt1: None,
            rect: None,
            preview: Drawable::empty(ctx),
        }
    }

    /// True if done
    fn event(&mut self, ctx: &mut EventCtx) -> bool {
        let pt = if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            pt
        } else {
            return false;
        };
        if let Some(pt1) = self.pt1 {
            if ctx.redo_mouseover() {
                self.rect = Polygon::rectangle_two_corners(pt1, pt);
                let mut batch = GeomBatch::new();
                if let Some(ref poly) = self.rect {
                    batch.push(Color::RED.alpha(0.5), poly.clone());
                }
                self.preview = batch.upload(ctx);
            }
            if ctx.input.left_mouse_button_released() {
                return true;
            }
        } else if ctx.input.left_mouse_button_pressed() {
            self.pt1 = Some(pt);
        }
        false
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.preview);
    }
}

struct Area {
    polygon: Polygon,
    borders: Vec<IntersectionID>,
    buildings: Vec<BuildingID>,
}

impl Area {
    fn new(app: &App, polygon: Polygon) -> Area {
        let mut borders = Vec::new();
        for i in app.primary.map.all_intersections() {
            if i.is_border() && polygon.contains_pt(i.polygon.center()) {
                borders.push(i.id);
            }
        }
        let mut buildings = Vec::new();
        for b in app.primary.map.all_buildings() {
            if polygon.contains_pt(b.polygon.center()) {
                buildings.push(b.id);
            }
        }
        Area {
            polygon,
            borders,
            buildings,
        }
    }
}
