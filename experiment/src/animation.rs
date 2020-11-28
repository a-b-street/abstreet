use geom::{Duration, Pt2D, Time};
use widgetry::{Drawable, EventCtx, GeomBatch, GfxCtx};

pub struct Animator {
    time: Time,
    active: Vec<Effect>,
    draw_current: Drawable,
}

struct Effect {
    start: Time,
    end: Time,
    orig: GeomBatch,
    center: Pt2D,
    // Scaling is the only transformation for now
    lerp_scale: (f64, f64),
}

impl Animator {
    pub fn new(ctx: &EventCtx) -> Animator {
        Animator {
            time: Time::START_OF_DAY,
            active: Vec::new(),
            draw_current: Drawable::empty(ctx),
        }
    }

    pub fn add(
        &mut self,
        duration: Duration,
        lerp_scale: (f64, f64),
        center: Pt2D,
        orig: GeomBatch,
    ) {
        self.active.push(Effect {
            start: self.time,
            end: self.time + duration,
            orig,
            lerp_scale,
            center,
        });
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if let Some(dt) = ctx.input.nonblocking_is_update_event() {
            self.time += dt;
            if self.active.is_empty() {
                return;
            }
            let mut batch = GeomBatch::new();
            let time = self.time;
            self.active.retain(|effect| effect.update(time, &mut batch));
            self.draw_current = ctx.upload(batch);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_current);
    }
}

impl Effect {
    fn update(&self, time: Time, batch: &mut GeomBatch) -> bool {
        let pct = (time - self.start) / (self.end - self.start);
        if pct > 1.0 {
            return false;
        }
        let scale = self.lerp_scale.0 + pct * (self.lerp_scale.1 - self.lerp_scale.0);
        batch.append(self.orig.clone().scale(scale).centered_on(self.center));
        true
    }
}
