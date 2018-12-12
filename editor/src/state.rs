use crate::plugins::debug::layers::ToggleableLayers;
use crate::plugins::debug::DebugMode;
use crate::plugins::edit::EditMode;
use crate::plugins::logs::DisplayLogs;
use crate::plugins::sim::SimMode;
use crate::plugins::time_travel::TimeTravel;
use crate::plugins::tutorial::TutorialMode;
use crate::plugins::view::ViewMode;
use crate::plugins::Plugin;
use crate::ui::PerMapUI;
use abstutil::Timer;
use ezgui::Canvas;
use sim::SimFlags;

// aka plugins that don't depend on map
pub struct PluginsPerUI {
    pub list: Vec<Box<Plugin>>,
}

impl PluginsPerUI {
    pub fn new(flags: &SimFlags) -> PluginsPerUI {
        let mut plugins = PluginsPerUI {
            list: vec![
                Box::new(EditMode::new()),
                Box::new(SimMode::new()),
                Box::new(DisplayLogs::new()),
            ],
        };

        // TODO Hacktastic way of sneaking this in!
        if flags.load == "../data/raw_maps/ban_left_turn.abst".to_string() {
            plugins.list.push(Box::new(TutorialMode::new()));
        }

        plugins
    }

    pub fn edit_mode(&self) -> &EditMode {
        self.list[0].downcast_ref::<EditMode>().unwrap()
    }

    pub fn sim_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }
}

pub struct PluginsPerMap {
    // Anything that holds onto any kind of ID has to live here!
    pub list: Vec<Box<Plugin>>,
}

impl PluginsPerMap {
    pub fn new(state: &PerMapUI, canvas: &Canvas, timer: &mut Timer) -> PluginsPerMap {
        let mut plugins = PluginsPerMap {
            list: vec![
                Box::new(DebugMode::new(&state.map)),
                Box::new(ViewMode::new(&state.map, &state.draw_map, timer)),
                Box::new(TimeTravel::new()),
            ],
        };
        plugins.layers_mut().handle_zoom(-1.0, canvas.cam_zoom);
        plugins
    }

    pub fn debug_mode(&self) -> &DebugMode {
        self.list[0].downcast_ref::<DebugMode>().unwrap()
    }

    pub fn view_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }

    pub fn time_travel(&self) -> &TimeTravel {
        self.list[2].downcast_ref::<TimeTravel>().unwrap()
    }

    pub fn layers(&self) -> &ToggleableLayers {
        &self.list[0].downcast_ref::<DebugMode>().unwrap().layers
    }

    pub fn layers_mut(&mut self) -> &mut ToggleableLayers {
        &mut self.list[0].downcast_mut::<DebugMode>().unwrap().layers
    }
}
