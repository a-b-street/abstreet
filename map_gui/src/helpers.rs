use geom::Polygon;
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, ParkingLotID, RoadID};
use sim::{AgentID, CarID, PedestrianID};
use widgetry::{GfxCtx, Line, Text};

use crate::AppLike;

#[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum ID {
    Road(RoadID),
    Lane(LaneID),
    Intersection(IntersectionID),
    Building(BuildingID),
    ParkingLot(ParkingLotID),
    Car(CarID),
    Pedestrian(PedestrianID),
    PedCrowd(Vec<PedestrianID>),
    BusStop(BusStopID),
    Area(AreaID),
}

impl ID {
    pub fn from_agent(id: AgentID) -> ID {
        match id {
            AgentID::Car(id) => ID::Car(id),
            AgentID::Pedestrian(id) => ID::Pedestrian(id),
            AgentID::BusPassenger(_, bus) => ID::Car(bus),
        }
    }

    pub fn agent_id(&self) -> Option<AgentID> {
        match *self {
            ID::Car(id) => Some(AgentID::Car(id)),
            ID::Pedestrian(id) => Some(AgentID::Pedestrian(id)),
            // PedCrowd doesn't map to a single agent.
            _ => None,
        }
    }
}

pub fn loading_tips() -> Text {
    Text::from_multiline(vec![
        Line("Recent changes (November 8)"),
        Line(""),
        Line("- Download more cities from within the game"),
        Line("- You can now click agents while zoomed out"),
        Line("- New OpenStreetMap viewer, open it from the splash screen"),
        Line("- A web version has launched!"),
        Line("- Slow segments of a trip shown in the info panel"),
        Line("- Alleyways are now included in the map"),
        Line("- Check out the trip tables and summary changes (press 'q')"),
        Line("- Try out the new traffic signal editor!"),
    ])
}

/// Make it clear the map can't be interacted with right now.
pub fn grey_out_map(g: &mut GfxCtx, app: &dyn AppLike) {
    g.fork_screenspace();
    // TODO - OSD height
    g.draw_polygon(
        app.cs().fade_map_dark,
        Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
    );
    g.unfork();
}
