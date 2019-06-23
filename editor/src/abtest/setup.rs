use crate::abtest::{ABTestMode, ABTestSavestate};
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use crate::render::DrawMap;
use crate::ui::{Flags, PerMapUI, UI};
use ezgui::{
    hotkey, EventCtx, EventLoopMode, GfxCtx, Key, LogScroller, ModalMenu, Wizard, WrappedWizard,
};
use geom::Duration;
use map_model::{Map, MapEdits};
use sim::{ABTest, Scenario, SimFlags};
use std::path::PathBuf;

pub struct PickABTest {
    wizard: Wizard,
}

impl PickABTest {
    pub fn new() -> PickABTest {
        PickABTest {
            wizard: Wizard::new(),
        }
    }
}

impl State for PickABTest {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if let Some(ab_test) = pick_ab_test(&ui.primary.map, self.wizard.wrap(ctx)) {
            let scroller = LogScroller::new(ab_test.test_name.clone(), ab_test.describe());
            return (
                Transition::Replace(Box::new(ABTestSetup {
                    menu: ModalMenu::new(
                        &format!("A/B Test Editor for {}", ab_test.test_name),
                        vec![
                            (hotkey(Key::Escape), "quit"),
                            (hotkey(Key::R), "run A/B test"),
                            (hotkey(Key::L), "load savestate"),
                        ],
                        ctx,
                    ),
                    ab_test,
                    scroller,
                })),
                EventLoopMode::InputOnly,
            );
        } else if self.wizard.aborted() {
            return (Transition::Pop, EventLoopMode::InputOnly);
        }
        (Transition::Keep, EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

struct ABTestSetup {
    menu: ModalMenu,
    ab_test: ABTest,
    scroller: LogScroller,
}

impl State for ABTestSetup {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        ctx.canvas.handle_event(ctx.input);
        self.menu.handle_event(ctx, None);
        if self.scroller.event(ctx.input) {
            return (Transition::Pop, EventLoopMode::InputOnly);
        } else if self.menu.action("run A/B test") {
            return (
                Transition::Replace(Box::new(launch_test(&self.ab_test, ui, ctx))),
                EventLoopMode::InputOnly,
            );
        } else if self.menu.action("load savestate") {
            return (
                Transition::Push(Box::new(LoadSavestate {
                    ab_test: self.ab_test.clone(),
                    wizard: Wizard::new(),
                })),
                EventLoopMode::InputOnly,
            );
        }
        (Transition::Keep, EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.scroller.draw(g);
        self.menu.draw(g);
    }
}

struct LoadSavestate {
    ab_test: ABTest,
    wizard: Wizard,
}

impl State for LoadSavestate {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if let Some(ss) = pick_savestate(&self.ab_test, &mut self.wizard.wrap(ctx)) {
            return (
                Transition::Replace(Box::new(launch_savestate(&self.ab_test, ss, ui, ctx))),
                EventLoopMode::InputOnly,
            );
        } else if self.wizard.aborted() {
            return (Transition::Pop, EventLoopMode::InputOnly);
        }
        (Transition::Keep, EventLoopMode::InputOnly)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

fn pick_ab_test(map: &Map, mut wizard: WrappedWizard) -> Option<ABTest> {
    let load_existing = "Load existing A/B test";
    let create_new = "Create new A/B test";
    if wizard.choose_string("What A/B test to manage?", vec![load_existing, create_new])?
        == load_existing
    {
        load_ab_test(map, &mut wizard, "Load which A/B test?")
    } else {
        let test_name = wizard.input_string("Name the A/B test")?;
        let ab_test = ABTest {
            test_name,
            map_name: map.get_name().to_string(),
            scenario_name: choose_scenario(map, &mut wizard, "What scenario to run?")?,
            edits1_name: choose_edits(map, &mut wizard, "For the 1st run, what map edits to use?")?,
            edits2_name: choose_edits(map, &mut wizard, "For the 2nd run, what map edits to use?")?,
        };
        ab_test.save();
        Some(ab_test)
    }
}

fn launch_test(test: &ABTest, ui: &mut UI, ctx: &mut EventCtx) -> ABTestMode {
    let secondary = ctx.loading_screen(
        &format!("Launching A/B test {}", test.test_name),
        |ctx, mut timer| {
            let load = PathBuf::from(format!(
                "../data/scenarios/{}/{}.bin",
                test.map_name, test.scenario_name
            ));
            if ui.primary.current_flags.sim_flags.rng_seed.is_none() {
                ui.primary.current_flags.sim_flags.rng_seed = Some(42);
            }

            {
                timer.start("load primary");
                ui.primary.current_flags.sim_flags.run_name =
                    Some(format!("{} with {}", test.test_name, test.edits1_name));
                apply_map_edits(
                    &mut ui.primary,
                    &ui.cs,
                    ctx,
                    MapEdits::load(&test.map_name, &test.edits1_name),
                );
                ui.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);

                let scenario: Scenario = abstutil::read_binary(load.to_str().unwrap(), &mut timer)
                    .expect("loading scenario failed");
                ui.primary.reset_sim();
                let mut rng = ui.primary.current_flags.sim_flags.make_rng();
                scenario.instantiate(&mut ui.primary.sim, &ui.primary.map, &mut rng, &mut timer);
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                timer.stop("load primary");
            }
            {
                timer.start("load secondary");
                let current_flags = &ui.primary.current_flags;
                // TODO We could try to be cheaper by cloning primary's Map, but cloning DrawMap
                // won't help -- we need to upload new stuff to the GPU. :\  The alternative is
                // doing apply_map_edits every time we swap.
                let mut secondary = PerMapUI::new(
                    Flags {
                        sim_flags: SimFlags {
                            load,
                            rng_seed: current_flags.sim_flags.rng_seed,
                            run_name: Some(format!("{} with {}", test.test_name, test.edits2_name)),
                        },
                        ..current_flags.clone()
                    },
                    &ui.cs,
                    ctx,
                    &mut timer,
                );
                apply_map_edits(
                    &mut secondary,
                    &ui.cs,
                    ctx,
                    MapEdits::load(&test.map_name, &test.edits2_name),
                );
                secondary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);
                secondary.sim.step(&secondary.map, Duration::seconds(0.1));
                timer.stop("load secondary");
                secondary
            }
        },
    );

    ABTestMode::new(ctx, ui, &test.test_name, secondary)
}

fn launch_savestate(test: &ABTest, ss_path: String, ui: &mut UI, ctx: &mut EventCtx) -> ABTestMode {
    ctx.loading_screen(
        &format!("Launch A/B test from savestate {}", ss_path),
        |ctx, mut timer| {
            let ss: ABTestSavestate = abstutil::read_binary(&ss_path, &mut timer).unwrap();

            timer.start("setup primary");
            ui.primary.map = ss.primary_map;
            ui.primary.sim = ss.primary_sim;
            ui.primary.draw_map = DrawMap::new(
                &ui.primary.map,
                &ui.primary.current_flags,
                &ui.cs,
                ctx.prerender,
                &mut timer,
            );
            timer.stop("setup primary");

            timer.start("setup secondary");
            let secondary = PerMapUI {
                draw_map: DrawMap::new(
                    &ss.secondary_map,
                    &ui.primary.current_flags,
                    &ui.cs,
                    ctx.prerender,
                    &mut timer,
                ),
                map: ss.secondary_map,
                sim: ss.secondary_sim,
                current_selection: None,
                // TODO Hack... can we just remove these?
                current_flags: ui.primary.current_flags.clone(),
            };
            timer.stop("setup secondary");

            ABTestMode::new(ctx, ui, &test.test_name, secondary)
        },
    )
}

fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(n, _)| n)
}

fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || {
                let mut list = abstutil::list_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), "no_edits".to_string()));
                list
            }),
        )
        .map(|(n, _)| n)
}

fn load_ab_test(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<ABTest> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<ABTest>(
            query,
            Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
        )
        .map(|(_, t)| t)
}

fn pick_savestate(test: &ABTest, wizard: &mut WrappedWizard) -> Option<String> {
    let path = format!(
        "../data/ab_test_saves/{}/{}/",
        test.map_name, test.test_name
    );
    wizard
        .choose_something_no_keys::<()>(
            "Load which savestate?",
            Box::new(move || {
                abstutil::list_dir(std::path::Path::new(&path))
                    .into_iter()
                    .map(|f| (f, ()))
                    .collect()
            }),
        )
        .map(|(f, _)| f)
}
