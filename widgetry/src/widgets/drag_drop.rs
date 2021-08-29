use crate::{
    Drawable, EventCtx, GeomBatch, GeomBatchStack, GfxCtx, Outcome, ScreenDims, ScreenPt,
    ScreenRectangle, Widget, WidgetImpl, WidgetOutput,
};

const SPACE_BETWEEN_CARDS: f64 = 2.0;

pub struct DragDrop<T: Copy + PartialEq> {
    label: String,
    cards: Vec<Card<T>>,
    draw: Drawable,
    state: State,
    dims: ScreenDims,
    top_left: ScreenPt,
}

struct Card<T: PartialEq> {
    value: T,
    dims: ScreenDims,
    default_batch: GeomBatch,
    hovering_batch: GeomBatch,
    selected_batch: GeomBatch,
}

#[derive(PartialEq)]
enum State {
    Initial {
        hovering: Option<usize>,
        selected: Option<usize>,
    },
    Idle {
        hovering: Option<usize>,
        selected: Option<usize>,
    },
    Dragging {
        orig_idx: usize,
        drag_from: ScreenPt,
        cursor_at: ScreenPt,
        new_idx: usize,
    },
}

impl<T: 'static + Copy + PartialEq> DragDrop<T> {
    /// This widget emits several events.
    ///
    /// - `Outcome::Changed(label)` when a different card is selected or hovered on
    /// - `Outcome::Changed("dragging " + label)` while dragging, when the drop position of the
    ///    card changes. Call `get_dragging_state` to learn the indices.
    /// - `Outcome::DragDropReleased` when a card is dropped
    pub fn new(ctx: &EventCtx, label: &str) -> Self {
        DragDrop {
            label: label.to_string(),
            cards: vec![],
            draw: Drawable::empty(ctx),
            state: State::Idle {
                hovering: None,
                selected: None,
            },
            dims: ScreenDims::zero(),
            top_left: ScreenPt::zero(),
        }
    }

    pub fn into_widget(mut self, ctx: &EventCtx) -> Widget {
        self.recalc_draw(ctx);
        Widget::new(Box::new(self))
    }

    pub fn selected_value(&self) -> Option<T> {
        let idx = match self.state {
            State::Initial { selected, .. } | State::Idle { selected, .. } => selected,
            State::Dragging { orig_idx, .. } => Some(orig_idx),
        }?;

        Some(self.cards[idx].value)
    }

    pub fn hovering_value(&self) -> Option<T> {
        let idx = match self.state {
            State::Initial { hovering, .. } | State::Idle { hovering, .. } => hovering,
            _ => None,
        }?;
        Some(self.cards[idx].value)
    }

    pub fn push_card(
        &mut self,
        value: T,
        dims: ScreenDims,
        default_batch: GeomBatch,
        hovering_batch: GeomBatch,
        selected_batch: GeomBatch,
    ) {
        self.cards.push(Card {
            value,
            dims,
            default_batch,
            hovering_batch,
            selected_batch,
        });
    }

    pub fn set_initial_state(&mut self, selected_value: Option<T>, hovering_value: Option<T>) {
        let selected = selected_value.and_then(|selected_value| {
            self.cards
                .iter()
                .position(|card| card.value == selected_value)
        });

        let hovering = hovering_value.and_then(|hovering_value| {
            self.cards
                .iter()
                .position(|card| card.value == hovering_value)
        });

        self.state = State::Initial { selected, hovering };
    }

    /// If a card is currently being dragged, return its original and (potential) new index.
    pub fn get_dragging_state(&self) -> Option<(usize, usize)> {
        match self.state {
            State::Dragging {
                orig_idx, new_idx, ..
            } => Some((orig_idx, new_idx)),
            _ => None,
        }
    }
}

impl<T: 'static + Copy + PartialEq> DragDrop<T> {
    fn recalc_draw(&mut self, ctx: &EventCtx) {
        let mut stack = GeomBatchStack::horizontal(Vec::new());
        stack.set_spacing(SPACE_BETWEEN_CARDS);

        let (dims, batch) = match self.state {
            State::Initial { hovering, selected } | State::Idle { hovering, selected } => {
                for (idx, card) in self.cards.iter().enumerate() {
                    if selected == Some(idx) {
                        stack.push(card.selected_batch.clone());
                    } else if hovering == Some(idx) {
                        stack.push(card.hovering_batch.clone());
                    } else {
                        stack.push(card.default_batch.clone());
                    }
                }
                let batch = stack.batch();
                (batch.get_dims(), batch)
            }
            State::Dragging {
                orig_idx,
                drag_from,
                cursor_at,
                new_idx,
            } => {
                let width = self.cards[orig_idx].dims.width;

                for (idx, card) in self.cards.iter().enumerate() {
                    // the target we're dragging
                    let batch = if idx == orig_idx {
                        card.selected_batch.clone()
                    } else if idx <= new_idx && idx > orig_idx {
                        // move batch to the left if target is newly greater than us
                        card.default_batch
                            .clone()
                            .translate(-(width + SPACE_BETWEEN_CARDS), 0.0)
                    } else if idx >= new_idx && idx < orig_idx {
                        // move batch to the right if target is newly less than us
                        card.default_batch
                            .clone()
                            .translate(width + SPACE_BETWEEN_CARDS, 0.0)
                    } else {
                        card.default_batch.clone()
                    };

                    stack.push(batch);
                }

                // PERF: avoid this clone by implementing a non-consuming `stack.get_dims()`.
                // At the moment it seems like not a big deal to just clone the thing
                let dims = stack.clone().batch().get_dims();

                // The dragged batch follows the cursor, but don't translate it until we've captured
                // the pre-existing `dims`, otherwise the dragged position will be included in the
                // overall dims of this widget, causing other screen content to shift around as we
                // drag.
                let mut dragged_batch = std::mem::take(stack.get_mut(orig_idx).unwrap());

                // offset the dragged item just a little to initially hint that it's moveable
                let floating_effect_offset = 4.0;
                dragged_batch = dragged_batch
                    .translate(
                        cursor_at.x - drag_from.x + floating_effect_offset,
                        cursor_at.y - drag_from.y - floating_effect_offset,
                    )
                    .set_z_offset(-0.1);
                *stack.get_mut(orig_idx).unwrap() = dragged_batch;

                (dims, stack.batch())
            }
        };
        self.dims = dims;
        self.draw = batch.upload(ctx);
    }

    fn mouseover_card(&self, ctx: &EventCtx) -> Option<usize> {
        let pt = ctx.canvas.get_cursor_in_screen_space()?;
        let mut top_left = self.top_left;
        for (idx, Card { dims, .. }) in self.cards.iter().enumerate() {
            if ScreenRectangle::top_left(top_left, *dims).contains(pt) {
                return Some(idx);
            }
            top_left.x += dims.width + SPACE_BETWEEN_CARDS;
        }
        None
    }
}

impl<T: 'static + Copy + PartialEq> WidgetImpl for DragDrop<T> {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        let new_state = match self.state {
            State::Initial { selected, hovering } => {
                if let Some(idx) = self.mouseover_card(ctx) {
                    if hovering != Some(idx) {
                        output.outcome = Outcome::Changed(self.label.clone());
                    }
                    State::Idle {
                        hovering: Some(idx),
                        selected,
                    }
                } else {
                    // Keep the intial state, which reflects hovering/selection from interacting
                    // with the lanes on the map.
                    return;
                }
            }
            State::Idle { hovering, selected } => match self.mouseover_card(ctx) {
                Some(idx) if ctx.input.left_mouse_button_pressed() => {
                    let cursor = ctx.canvas.get_cursor_in_screen_space().unwrap();
                    State::Dragging {
                        orig_idx: idx,
                        drag_from: cursor,
                        cursor_at: cursor,
                        new_idx: idx,
                    }
                }
                maybe_idx => {
                    if hovering != maybe_idx {
                        output.outcome = Outcome::Changed(self.label.clone());
                    }
                    State::Idle {
                        hovering: maybe_idx,
                        selected,
                    }
                }
            },
            State::Dragging {
                orig_idx,
                new_idx,
                cursor_at,
                drag_from,
            } => {
                if ctx.input.left_mouse_button_released() {
                    output.outcome =
                        Outcome::DragDropReleased(self.label.clone(), orig_idx, new_idx);
                    if orig_idx != new_idx {
                        let item = self.cards.remove(orig_idx);
                        self.cards.insert(new_idx, item);
                    }

                    State::Idle {
                        hovering: Some(new_idx),
                        selected: Some(new_idx),
                    }
                } else {
                    // TODO https://jqueryui.com/sortable/ only swaps once you cross the center of
                    // the new card
                    let updated_idx = self.mouseover_card(ctx).unwrap_or(new_idx);
                    if new_idx != updated_idx {
                        output.outcome = Outcome::Changed(format!("dragging {}", self.label));
                    }

                    State::Dragging {
                        orig_idx,
                        new_idx: updated_idx,
                        cursor_at: ctx.canvas.get_cursor_in_screen_space().unwrap_or(cursor_at),
                        drag_from,
                    }
                }
            }
        };

        if self.state != new_state {
            self.state = new_state;
            self.recalc_draw(ctx);
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}
