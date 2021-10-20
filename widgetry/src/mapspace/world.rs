use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use aabb_quadtree::{ItemId, QuadTree};

use geom::{Bounds, Circle, Distance, Polygon, Pt2D};

use crate::mapspace::{ToggleZoomed, ToggleZoomedBuilder};
use crate::{Color, EventCtx, GeomBatch, GfxCtx, MultiKey, RewriteColor, Text};
use crate::mapspace::unzoomed::UnzoomedCircle;

// TODO Tests...
// - start drag in screenspace, release in map
// - start drag in mapspace, release in screen
// - reset hovering when we go out of screenspace
// - start dragging one object, and while dragging, hover on top of other objects

/// A `World` manages objects that exist in "map-space", the zoomable and pannable canvas. These
/// objects can be drawn, hovered on, clicked, dragged, etc.
pub struct World<ID: ObjectID> {
    // TODO Hashing may be too slow in some cases
    objects: HashMap<ID, Object<ID>>,
    quadtree: QuadTree<ID>,

    draw_master_batches: Vec<ToggleZoomed>,

    hovering: Option<ID>,
    // If we're currently dragging, where was the cursor during the last movement, and has the
    // cursor moved since starting the drag?
    dragging_from: Option<(Pt2D, bool)>,
}

/// The result of a `World` handling an event
pub enum WorldOutcome<ID: ObjectID> {
    /// A left click occurred while not hovering on any object
    ClickedFreeSpace(Pt2D),
    /// An object is being dragged. The given offsets are relative to the previous dragging event.
    /// The current position of the cursor is included.
    Dragging {
        obj: ID,
        dx: f64,
        dy: f64,
        cursor: Pt2D,
    },
    /// While hovering on an object with a defined hotkey, that key was pressed.
    Keypress(&'static str, ID),
    /// A hoverable object was clicked
    ClickedObject(ID),
    /// Nothing interesting happened
    Nothing,
}

impl<I: ObjectID> WorldOutcome<I> {
    /// If the outcome references some ID, transform it to another type. This is useful when some
    /// component owns a World that contains a few different types of objects, some of which are
    /// managed by another component that only cares about its IDs.
    pub fn map_id<O: ObjectID, F: Fn(I) -> O>(self, f: F) -> WorldOutcome<O> {
        match self {
            WorldOutcome::ClickedFreeSpace(pt) => WorldOutcome::ClickedFreeSpace(pt),
            WorldOutcome::Dragging {
                obj,
                dx,
                dy,
                cursor,
            } => WorldOutcome::Dragging {
                obj: f(obj),
                dx,
                dy,
                cursor,
            },
            WorldOutcome::Keypress(action, id) => WorldOutcome::Keypress(action, f(id)),
            WorldOutcome::ClickedObject(id) => WorldOutcome::ClickedObject(f(id)),
            WorldOutcome::Nothing => WorldOutcome::Nothing,
        }
    }
}

/// Objects in a `World` are uniquely identified by this caller-specified type
pub trait ObjectID: Clone + Copy + Debug + Eq + Hash {}

/// This provides a builder API for adding objects to a `World`.
pub struct ObjectBuilder<'a, ID: ObjectID> {
    world: &'a mut World<ID>,

    id: ID,

    // For regular map-space things
    hitbox: Option<Polygon>,
    draw_normal: Option<ToggleZoomedBuilder>,
    draw_hover: Option<ToggleZoomedBuilder>,

    // For unzoomed circles
    // TODO Enums
    unzoomed_circle: Option<UnzoomedCircle>,

    zorder: usize,
    tooltip: Option<Text>,
    clickable: bool,
    draggable: bool,
    keybindings: Vec<(MultiKey, &'static str)>,
}

impl<'a, ID: ObjectID> ObjectBuilder<'a, ID> {
    /// Specifies the geometry of the object. Required.
    pub fn hitbox(mut self, polygon: Polygon) -> Self {
        assert!(self.hitbox.is_none(), "called hitbox twice");
        self.hitbox = Some(polygon);
        self
    }

    /// Provides ordering for overlapping objects. Higher values are "on top" of lower values.
    pub fn zorder(mut self, zorder: usize) -> Self {
        assert!(self.zorder == 0, "called zorder twice");
        self.zorder = zorder;
        self
    }

    /// Specifies how to draw this object normally (while not hovering on it)
    pub fn draw<I: Into<ToggleZoomedBuilder>>(mut self, normal: I) -> Self {
        assert!(
            self.draw_normal.is_none(),
            "already specified how to draw normally"
        );
        self.draw_normal = Some(normal.into());
        self
    }

    /// Draw the object by coloring its hitbox
    pub fn draw_color(self, color: Color) -> Self {
        let hitbox = self.hitbox.clone().expect("call hitbox first");
        self.draw(GeomBatch::from(vec![(color, hitbox)]))
    }

    /// Indicate that an object doesn't need to be drawn individually. A call to `draw_master_batch` covers it.
    pub fn drawn_in_master_batch(self) -> Self {
        assert!(
            self.draw_normal.is_none(),
            "object is already drawn normally"
        );
        self.draw(GeomBatch::new())
    }

    /// Specifies how to draw the object while the cursor is hovering on it. Note that an object
    /// isn't considered hoverable unless this is specified!
    pub fn draw_hovered<I: Into<ToggleZoomedBuilder>>(mut self, hovered: I) -> Self {
        assert!(
            self.draw_hover.is_none(),
            "already specified how to draw hovered"
        );
        self.draw_hover = Some(hovered.into());
        self
    }

    /// Draw the object in a hovered state by transforming the normal drawing.
    pub fn draw_hover_rewrite(self, rewrite: RewriteColor) -> Self {
        let hovered = self
            .draw_normal
            .clone()
            .expect("first specify how to draw normally")
            .color(rewrite);
        self.draw_hovered(hovered)
    }

    /// Draw the object in a hovered state by changing the alpha value of the normal drawing.
    pub fn hover_alpha(self, alpha: f32) -> Self {
        self.draw_hover_rewrite(RewriteColor::ChangeAlpha(alpha))
    }

    /// Draw a tooltip while hovering over this object.
    pub fn tooltip(mut self, txt: Text) -> Self {
        assert!(self.tooltip.is_none(), "already specified tooltip");
        // TODO Or should this implicitly mark the object as hoverable? Is it weird to base this
        // off drawing?
        assert!(
            self.draw_hover.is_some(),
            "first specify how to draw hovered"
        );
        self.tooltip = Some(txt);
        self
    }

    /// Mark the object as clickable. `WorldOutcome::ClickedObject` will be fired.
    pub fn clickable(mut self) -> Self {
        assert!(!self.clickable, "called clickable twice");
        self.clickable = true;
        self
    }

    /// Mark the object as draggable. The user can hover on this object, then click and drag it.
    /// `WorldOutcome::Dragging` events will be fired.
    ///
    /// Note that dragging an object doesn't transform it at all (for example, by translating its
    /// hitbox). The caller is responsible for doing that.
    pub fn draggable(mut self) -> Self {
        assert!(!self.draggable, "called draggable twice");
        self.draggable = true;
        self
    }

    /// While the user hovers over this object, they can press a key to perform the specified
    /// action. `WorldOutcome::Keypress` will be fired.
    pub fn hotkey<I: Into<MultiKey>>(mut self, key: I, action: &'static str) -> Self {
        // TODO Check for duplicate keybindings
        self.keybindings.push((key.into(), action));
        self
    }

    /// Finalize the object, adding it to the `World`.
    pub fn build(mut self, ctx: &mut EventCtx) {
        // TODO what do we want to put in the quadtree in this case? do we just have two different
        // quadtrees or strategies? Maybe we treat the objects as separate, merge both lists when
        // needed, stop trying to shoehorn into one thing.
        let body = if let Some(circle) = self.unzoomed_circle

        let hitbox = self.hitbox.take().expect("didn't specify hitbox");
        let bounds = hitbox.get_bounds();
        let quadtree_id = self
            .world
            .quadtree
            .insert_with_box(self.id, bounds.as_bbox());

        self.world.objects.insert(
            self.id,
            Object {
                _id: self.id,
                _quadtree_id: quadtree_id,
                hitbox,
                zorder: self.zorder,
                draw_normal: self
                    .draw_normal
                    .expect("didn't specify how to draw normally")
                    .build(ctx),
                draw_hover: self.draw_hover.take().map(|draw| draw.build(ctx)),
                tooltip: self.tooltip,
                clickable: self.clickable,
                draggable: self.draggable,
                keybindings: self.keybindings,
            },
        );
    }
}

struct Object<ID: ObjectID> {
    _id: ID,
    _quadtree_id: ItemId,

    body: Body,
    zorder: usize,
    tooltip: Option<Text>,
    clickable: bool,
    draggable: bool,
    // TODO How should we communicate these keypresses are possible? Something standard, like
    // button tooltips?
    keybindings: Vec<(MultiKey, &'static str)>,
}

enum Body {
    MapspacePrebaked {
        hitbox: Polygon,
        draw_normal: ToggleZoomed,
        draw_hover: Option<ToggleZoomed>,
    },
    // TODO We can have mapspace stuff that lazily calculates draw_hover and caches or something
    Circle {
        circle: UnzoomedCircle,
    },
}

impl Body {
    fn is_hoverable(&self) -> bool {
        match self {
            Body::MapspacePrebaked { ref draw_hover, .. } => draw_hover.is_some(),
            Body::Circle { .. } => true,
        }
    }

    fn contains_pt(&self, ctx: &EventCtx, pt: Pt2D) -> bool {
        match self {
            Body::MapspacePrebaked { ref hitbox, .. } => hitbox.contains_pt(pt),
            Body::Circle {ref circle } => Circle::new(circle.pt, circle.radius / ctx.canvas.cam_zoom).contains_pt(pt),
        }
    }

    fn draw_normal(&self, g: &mut GfxCtx) {
        match self {
            Body::MapspacePrebaked { ref draw_normal, .. } => g.redraw(draw_normal),
            Body::Circle { ref circle } => {
                // TODO Do something more efficient
                g.draw_polygon(circle.color, Circle::new(circle.pt, circle.radius / g.canvas.cam_zoom).to_polygon());
            }
        }
    }

    fn draw_hovered(&self, g: &mut GfxCtx) {
        match self {
            // draw_hovered must be set if the object is being hovered on
            Body::MapspacePrebaked { ref draw_hovered, .. } => g.redraw(draw_hovered.unwrap()),
            Body::Circle { ref circle } => {
                // TODO Do something more efficient
                g.draw_polygon(circle.color, Circle::new(circle.pt, circle.radius / g.canvas.cam_zoom).to_polygon());
            }
        }
    }
}

impl<ID: ObjectID> World<ID> {
    /// Creates an empty `World`, whose objects can exist anywhere from (0, 0) to the max f64.
    pub fn unbounded() -> World<ID> {
        World {
            objects: HashMap::new(),
            quadtree: QuadTree::default(
                Bounds::from(&[Pt2D::new(0.0, 0.0), Pt2D::new(std::f64::MAX, std::f64::MAX)])
                    .as_bbox(),
            ),

            draw_master_batches: Vec::new(),

            hovering: None,
            dragging_from: None,
        }
    }

    /// Creates an empty `World`, whose objects can exist in the provided rectangular boundary.
    pub fn bounded(bounds: &Bounds) -> World<ID> {
        World {
            objects: HashMap::new(),
            quadtree: QuadTree::default(bounds.as_bbox()),

            draw_master_batches: Vec::new(),

            hovering: None,
            dragging_from: None,
        }
    }

    /// Start adding an object to the `World`. The caller should specify the object with methods on
    /// `ObjectBuilder`, then call `build`.
    pub fn add(&mut self, id: ID) -> ObjectBuilder<'_, ID> {
        assert!(!self.objects.contains_key(&id), "duplicate object added");
        ObjectBuilder {
            world: self,

            hitbox: None,
            draw_normal: None,
            draw_hover: None,
            unzoomed_circle: None,

            id,
            zorder: 0,
            tooltip: None,
            clickable: false,
            draggable: false,
            keybindings: Vec::new(),
        }
    }

    /// After adding all objects to a `World`, call this to initially detect if the cursor is
    /// hovering on an object.
    pub fn initialize_hover(&mut self, ctx: &EventCtx) {
        self.hovering = ctx
            .canvas
            .get_cursor_in_map_space()
            .and_then(|cursor| self.calculate_hover(ctx, cursor));
    }

    /// If a drag event causes the world to be totally rebuilt, call this with the previous world
    /// to preserve the ongoing drag.
    ///
    /// This should be called after `initialize_hover`.
    ///
    /// Important: the rebuilt world must include the same object ID that's currently being dragged
    /// from the previous world.
    pub fn rebuilt_during_drag(&mut self, prev_world: &World<ID>) {
        if prev_world.dragging_from.is_some() {
            self.dragging_from = prev_world.dragging_from;
            self.hovering = prev_world.hovering;
            assert!(self.objects.contains_key(self.hovering.as_ref().unwrap()));
        }
    }

    /// Draw something underneath all objects. This is useful for performance, when a large number
    /// of objects never change appearance.
    pub fn draw_master_batch<I: Into<ToggleZoomedBuilder>>(&mut self, ctx: &EventCtx, draw: I) {
        self.draw_master_batches.push(draw.into().build(ctx));
    }

    /// Let objects in the world respond to something happening.
    pub fn event(&mut self, ctx: &mut EventCtx) -> WorldOutcome<ID> {
        if let Some((drag_from, moved)) = self.dragging_from {
            if ctx.input.left_mouse_button_released() {
                self.dragging_from = None;
                // For objects that're both clickable and draggable, we don't know what the user is
                // doing until they release the mouse!
                if !moved && self.objects[&self.hovering.unwrap()].clickable {
                    return WorldOutcome::ClickedObject(self.hovering.unwrap());
                }

                self.hovering = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|cursor| self.calculate_hover(ctx, cursor));
                return WorldOutcome::Nothing;
            }
            // Allow zooming, but not panning, while dragging
            if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
            }

            if ctx.redo_mouseover() {
                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    let dx = cursor.x() - drag_from.x();
                    let dy = cursor.y() - drag_from.y();
                    self.dragging_from = Some((cursor, true));
                    return WorldOutcome::Dragging {
                        obj: self.hovering.unwrap(),
                        dx,
                        dy,
                        cursor,
                    };
                }
            }

            return WorldOutcome::Nothing;
        }

        let cursor = if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            pt
        } else {
            self.hovering = None;
            return WorldOutcome::Nothing;
        };

        // Possibly recalculate hovering
        if ctx.redo_mouseover() {
            self.hovering = self.calculate_hover(ctx, cursor);
        }

        // If we're hovering on a draggable thing, only allow zooming, not panning
        let mut allow_panning = true;
        if let Some(id) = self.hovering {
            let obj = &self.objects[&id];

            // For objects both clickable and draggable, the branch below will win, and we'll
            // detect a normal click elsewhere.
            if obj.clickable && ctx.normal_left_click() {
                return WorldOutcome::ClickedObject(id);
            }

            if obj.draggable {
                allow_panning = false;
                if ctx.input.left_mouse_button_pressed() {
                    self.dragging_from = Some((cursor, false));
                    return WorldOutcome::Nothing;
                }
            }

            for (key, action) in &obj.keybindings {
                if ctx.input.pressed(key.clone()) {
                    return WorldOutcome::Keypress(action, id);
                }
            }
        }

        if allow_panning {
            ctx.canvas_movement();

            if self.hovering.is_none() && ctx.normal_left_click() {
                return WorldOutcome::ClickedFreeSpace(cursor);
            }
        } else if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
            ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
        }

        WorldOutcome::Nothing
    }

    fn calculate_hover(&self, ctx: &EventCtx, cursor: Pt2D) -> Option<ID> {
        let mut objects = Vec::new();
        for &(id, _, _) in &self.quadtree.query(
            // Maybe worth tuning. Since we do contains_pt below, it doesn't matter if this is too
            // big; just a performance impact possibly.
            Circle::new(cursor, Distance::meters(3.0))
                .get_bounds()
                .as_bbox(),
        ) {
            objects.push(*id);
        }
        objects.sort_by_key(|id| self.objects[id].zorder);
        objects.reverse();

        for id in objects {
            let obj = &self.objects[&id];
            if obj.is_hoverable() && obj.contains_pt(ctx, cursor) {
                return Some(id);
            }
        }
        None
    }

    /// Draw objects in the world that're currently visible.
    pub fn draw(&self, g: &mut GfxCtx) {
        // Always draw master batches first
        for draw in &self.draw_master_batches {
            draw.draw(g);
        }

        let mut objects = Vec::new();
        for &(id, _, _) in &self.quadtree.query(g.get_screen_bounds().as_bbox()) {
            objects.push(*id);
        }
        objects.sort_by_key(|id| self.objects[id].zorder);

        for id in objects {
            let obj = &self.objects[&id];
            if Some(id) == self.hovering {
                obj.body.draw_hovered(g);
                if let Some(ref txt) = obj.tooltip {
                    g.draw_mouse_tooltip(txt.clone());
                }
            } else {
                obj.body.draw_normal(g);
            }
        }
    }
}

/// If you don't ever need to refer to objects in a `World`, you can auto-assign dummy IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DummyID(usize);
impl ObjectID for DummyID {}

impl World<DummyID> {
    /// Begin adding an unnamed object to the `World`.
    ///
    /// Note: You must call `build` on this object before calling `add_unnamed` again. Otherwise,
    /// the object IDs will collide.
    pub fn add_unnamed(&mut self) -> ObjectBuilder<'_, DummyID> {
        self.add(DummyID(self.objects.len()))
    }
}
