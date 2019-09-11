use ezgui::{Color, EventCtx, EventLoopMode, GfxCtx, Key, Text, Wizard, GUI};
use geom::{Distance, Line};
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use std::{env, process};
use synthetic::{BuildingID, Direction, Model, ID};

struct UI {
    model: Model,
    state: State,
    osd: Text,
}

enum State {
    Viewing,
    MovingIntersection(StableIntersectionID),
    MovingBuilding(BuildingID),
    LabelingBuilding(BuildingID, Wizard),
    LabelingRoad((StableRoadID, Direction), Wizard),
    LabelingIntersection(StableIntersectionID, Wizard),
    CreatingRoad(StableIntersectionID),
    EditingRoad(StableRoadID, Wizard),
    SavingModel(Wizard),
}

impl UI {
    fn new(load: Option<&String>, exclude_bldgs: bool, ctx: &EventCtx) -> UI {
        let model = if let Some(path) = load {
            Model::import(path, exclude_bldgs, ctx.prerender)
        } else {
            Model::new()
        };
        UI {
            model,
            state: State::Viewing,
            osd: Text::new(),
        }
    }
}

impl GUI for UI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        ctx.canvas.handle_event(ctx.input);
        self.model.handle_mouseover(ctx);

        let cursor = {
            if let Some(c) = ctx.canvas.get_cursor_in_map_space() {
                c
            } else {
                return EventLoopMode::InputOnly;
            }
        };

        match self.state {
            State::MovingIntersection(id) => {
                self.model.move_i(id, cursor);
                if ctx.input.key_released(Key::LeftControl) {
                    self.state = State::Viewing;
                }
            }
            State::MovingBuilding(id) => {
                self.model.move_b(id, cursor);
                if ctx.input.key_released(Key::LeftControl) {
                    self.state = State::Viewing;
                }
            }
            State::LabelingBuilding(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label the building",
                    self.model.get_b_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_b_label(id, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingRoad(pair, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label this side of the road",
                    self.model.get_r_label(pair).unwrap_or_else(String::new),
                ) {
                    self.model.set_r_label(pair, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::LabelingIntersection(id, ref mut wizard) => {
                if let Some(label) = wizard.wrap(ctx).input_string_prefilled(
                    "Label the intersection",
                    self.model.get_i_label(id).unwrap_or_else(String::new),
                ) {
                    self.model.set_i_label(id, label);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::CreatingRoad(i1) => {
                if ctx.input.key_pressed(Key::Escape, "stop defining road") {
                    self.state = State::Viewing;
                } else if let Some(ID::Intersection(i2)) = self.model.get_selection() {
                    if i1 != i2 && ctx.input.key_pressed(Key::R, "finalize road") {
                        self.model.create_road(i1, i2);
                        self.state = State::Viewing;
                    }
                }
            }
            State::EditingRoad(id, ref mut wizard) => {
                if let Some(s) = wizard
                    .wrap(ctx)
                    .input_string_prefilled("Specify the lanes", self.model.get_lanes(id))
                {
                    self.model.edit_lanes(id, s);
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::SavingModel(ref mut wizard) => {
                if let Some(name) = wizard.wrap(ctx).input_string("Name the synthetic map") {
                    self.model.name = Some(name);
                    self.model.export();
                    self.state = State::Viewing;
                } else if wizard.aborted() {
                    self.state = State::Viewing;
                }
            }
            State::Viewing => {
                if let Some(ID::Intersection(i)) = self.model.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move intersection") {
                        self.state = State::MovingIntersection(i);
                    } else if ctx.input.key_pressed(Key::R, "create road") {
                        self.state = State::CreatingRoad(i);
                    } else if ctx.input.key_pressed(Key::Backspace, "delete intersection") {
                        self.model.remove_i(i);
                    } else if ctx.input.key_pressed(Key::T, "toggle intersection type") {
                        self.model.toggle_i_type(i);
                    } else if ctx.input.key_pressed(Key::L, "label intersection") {
                        self.state = State::LabelingIntersection(i, Wizard::new());
                    }
                } else if let Some(ID::Building(b)) = self.model.get_selection() {
                    if ctx.input.key_pressed(Key::LeftControl, "move building") {
                        self.state = State::MovingBuilding(b);
                    } else if ctx.input.key_pressed(Key::Backspace, "delete building") {
                        self.model.remove_b(b);
                    } else if ctx.input.key_pressed(Key::L, "label building") {
                        self.state = State::LabelingBuilding(b, Wizard::new());
                    }
                } else if let Some(ID::Lane(r, dir, _)) = self.model.get_selection() {
                    if ctx
                        .input
                        .key_pressed(Key::Backspace, &format!("delete road {}", r))
                    {
                        self.model.remove_road(r);
                    } else if ctx.input.key_pressed(Key::E, "edit lanes") {
                        self.state = State::EditingRoad(r, Wizard::new());
                    } else if ctx.input.key_pressed(Key::S, "swap lanes") {
                        self.model.swap_lanes(r);
                    } else if ctx.input.key_pressed(Key::L, "label side of the road") {
                        self.state = State::LabelingRoad((r, dir), Wizard::new());
                    }
                } else if ctx.input.unimportant_key_pressed(Key::Escape, "quit") {
                    process::exit(0);
                } else if ctx.input.key_pressed(Key::S, "save") {
                    if self.model.name.is_some() {
                        self.model.export();
                    } else {
                        self.state = State::SavingModel(Wizard::new());
                    }
                } else if ctx.input.key_pressed(Key::I, "create intersection") {
                    self.model.create_i(cursor);
                } else if ctx.input.key_pressed(Key::B, "create building") {
                    self.model.create_b(cursor);
                }
            }
        }

        self.osd = Text::new();
        ctx.input.populate_osd(&mut self.osd);
        EventLoopMode::InputOnly
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.model.draw(g);

        match self.state {
            State::CreatingRoad(i1) => {
                if let Some(cursor) = g.get_cursor_in_map_space() {
                    g.draw_line(
                        Color::GREEN,
                        Distance::meters(5.0),
                        &Line::new(self.model.get_i_center(i1), cursor),
                    );
                }
            }
            State::LabelingBuilding(_, ref wizard)
            | State::LabelingRoad(_, ref wizard)
            | State::LabelingIntersection(_, ref wizard)
            | State::EditingRoad(_, ref wizard)
            | State::SavingModel(ref wizard) => {
                wizard.draw(g);
            }
            _ => {}
        };

        g.draw_blocking_text(&self.osd, ezgui::BOTTOM_LEFT);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    ezgui::run("Synthetic map editor", 1024.0, 768.0, |ctx| {
        UI::new(
            args.get(1),
            args.get(2) == Some(&"--nobldgs".to_string()),
            ctx,
        )
    });
}
