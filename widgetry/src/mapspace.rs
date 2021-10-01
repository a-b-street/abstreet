use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use aabb_quadtree::{ItemId, QuadTree};

use geom::{Bounds, Circle, Distance, Polygon, Pt2D};

use crate::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, MultiKey, RewriteColor};

// TODO Tests...
// - start drag in screenspace, release in map
// - start drag in mapspace, release in screen
// - reset hovering when we go out of screenspace

/// A `World` manages objects that exist in "map-space", the zoomable and pannable canvas. These
/// objects can be drawn, hovered on, clicked, dragged, etc.
pub struct World<ID: ObjectID> {
    // TODO Hashing may be too slow in some cases
    objects: HashMap<ID, Object<ID>>,
    quadtree: QuadTree<ID>,

    hovering: Option<ID>,
    // If we're currently dragging, where was the cursor during the last movement?
    dragging_from: Option<Pt2D>,
}

/// The result of a `World` handling an event
pub enum WorldOutcome<ID: ObjectID> {
    /// A left click occurred while not hovering on any object
    ClickedFreeSpace(Pt2D),
    /// An object is being dragged. The given offsets are relative to the previous dragging event.
    Dragging { obj: ID, dx: f64, dy: f64 },
    /// While hovering on an object with a defined hotkey, that key was pressed.
    Keypress(&'static str, ID),
    /// Nothing interesting happened
    Nothing,
}

/// Objects in a `World` are uniquely identified by this caller-specified type
pub trait ObjectID: Clone + Copy + Debug + Eq + Hash {}

/// This provides a builder API for adding objects to a `World`.
pub struct ObjectBuilder<'a, ID: ObjectID> {
    world: &'a mut World<ID>,

    id: ID,
    hitbox: Option<Polygon>,
    zorder: usize,
    draw_normal: Option<GeomBatch>,
    draw_hover: Option<GeomBatch>,
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
    pub fn draw(mut self, batch: GeomBatch) -> Self {
        assert!(
            self.draw_normal.is_none(),
            "already specified how to draw normally"
        );
        self.draw_normal = Some(batch);
        self
    }

    /// Draw the object by coloring its hitbox
    pub fn draw_color(self, color: Color) -> Self {
        let hitbox = self.hitbox.clone().expect("call hitbox first");
        self.draw(GeomBatch::from(vec![(color, hitbox)]))
    }

    /// Specifies how to draw the object while the cursor is hovering on it. Note that an object
    /// isn't considered hoverable unless this is specified!
    pub fn draw_hovered(mut self, batch: GeomBatch) -> Self {
        assert!(
            self.draw_hover.is_none(),
            "already specified how to draw hovered"
        );
        self.draw_hover = Some(batch);
        self
    }

    /// Draw the object in a hovered state by changing the alpha value of the normal drawing.
    pub fn hover_alpha(self, alpha: f32) -> Self {
        let batch = self
            .draw_normal
            .clone()
            .expect("first specify how to draw normally")
            .color(RewriteColor::ChangeAlpha(alpha));
        self.draw_hovered(batch)
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
                draw_normal: ctx.upload(
                    self.draw_normal
                        .expect("didn't specify how to draw normally"),
                ),
                draw_hover: self.draw_hover.take().map(|batch| ctx.upload(batch)),
                draggable: self.draggable,
                keybindings: self.keybindings,
            },
        );
    }
}

struct Object<ID: ObjectID> {
    _id: ID,
    _quadtree_id: ItemId,
    hitbox: Polygon,
    zorder: usize,
    draw_normal: Drawable,
    draw_hover: Option<Drawable>,
    draggable: bool,
    // TODO How should we communicate these keypresses are possible? Something standard, like
    // button tooltips?
    keybindings: Vec<(MultiKey, &'static str)>,
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

            hovering: None,
            dragging_from: None,
        }
    }

    /// Creates an empty `World`, whose objects can exist in the provided rectangular boundary.
    pub fn bounded(bounds: &Bounds) -> World<ID> {
        World {
            objects: HashMap::new(),
            quadtree: QuadTree::default(bounds.as_bbox()),

            hovering: None,
            dragging_from: None,
        }
    }

    /// Start adding an object to the `World`. The caller should specify the object with methods on
    /// `ObjectBuilder`, then call `build`.
    pub fn add<'a>(&'a mut self, id: ID) -> ObjectBuilder<'a, ID> {
        assert!(!self.objects.contains_key(&id), "duplicate object added");
        ObjectBuilder {
            world: self,

            id,
            hitbox: None,
            zorder: 0,
            draw_normal: None,
            draw_hover: None,
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
            .and_then(|cursor| self.calculate_hover(cursor));
    }

    /// If a drag event causes the world to be totally rebuilt, call this with the previous world
    /// to preserve the ongoing drag.
    pub fn rebuilt_during_drag(&mut self, prev_world: &World<ID>) {
        self.dragging_from = prev_world.dragging_from;
    }

    /// Let objects in the world respond to something happening.
    pub fn event(&mut self, ctx: &mut EventCtx) -> WorldOutcome<ID> {
        if let Some(drag_from) = self.dragging_from {
            if ctx.input.left_mouse_button_released() {
                self.dragging_from = None;
                self.hovering = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|cursor| self.calculate_hover(cursor));
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
                    self.dragging_from = Some(cursor);
                    return WorldOutcome::Dragging {
                        obj: self.hovering.unwrap(),
                        dx,
                        dy,
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
            self.hovering = self.calculate_hover(cursor);
        }

        // If we're hovering on a draggable thing, only allow zooming, not panning
        let mut allow_panning = true;
        if let Some(id) = self.hovering {
            let obj = &self.objects[&id];

            if obj.draggable {
                allow_panning = false;
                if ctx.input.left_mouse_button_pressed() {
                    self.dragging_from = Some(cursor);
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
        } else {
            if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
            }
        }

        WorldOutcome::Nothing
    }

    fn calculate_hover(&self, cursor: Pt2D) -> Option<ID> {
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
            if obj.draw_hover.is_some() && obj.hitbox.contains_pt(cursor) {
                return Some(id);
            }
        }
        None
    }

    /// Draw objects in the world that're currently visible.
    pub fn draw(&self, g: &mut GfxCtx) {
        let mut objects = Vec::new();
        for &(id, _, _) in &self.quadtree.query(g.get_screen_bounds().as_bbox()) {
            objects.push(*id);
        }
        objects.sort_by_key(|id| self.objects[id].zorder);

        for id in objects {
            if Some(id) == self.hovering {
                if let Some(ref draw) = self.objects[&id].draw_hover {
                    g.redraw(draw);
                    continue;
                }
            }
            g.redraw(&self.objects[&id].draw_normal);
        }
    }
}
