//! Several distinct tools/applications all share the same general structure for their shared GUI
//! state, based around drawing and interacting with a Map.

pub mod colors;
pub mod common;
pub mod helpers;
pub mod options;
pub mod render;

/// Why not use composition and put the Map, DrawMap, etc in a struct? I think it wouldn't let us
/// have any common widgetry States... although maybe we can instead organize the common state into
/// a struct, and make the trait we pass around just be a getter/setter for this shared struct?
pub trait AppLike {
    fn map(&self) -> &map_model::Map;
    fn sim(&self) -> &sim::Sim;
    fn cs(&self) -> &colors::ColorScheme;
    fn draw_map(&self) -> &render::DrawMap;
    fn mut_draw_map(&mut self) -> &mut render::DrawMap;
    fn opts(&self) -> &options::Options;
    fn mut_opts(&mut self) -> &mut options::Options;
    fn unzoomed_agents(&self) -> &render::UnzoomedAgents;
    /// Change the color scheme. Idempotent. Return true if there was a change.
    fn change_color_scheme(
        &mut self,
        ctx: &mut widgetry::EventCtx,
        cs: colors::ColorSchemeChoice,
    ) -> bool;
}
