use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use geom::{Circle, Distance, Duration, Pt2D, Time};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};

pub struct Animator {
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
            active: Vec::new(),
            draw_current: Drawable::empty(ctx),
        }
    }

    pub fn add(
        &mut self,
        now: Time,
        duration: Duration,
        lerp_scale: (f64, f64),
        center: Pt2D,
        orig: GeomBatch,
    ) {
        self.active.push(Effect {
            start: now,
            end: now + duration,
            orig,
            lerp_scale,
            center,
        });
    }

    pub fn event(&mut self, ctx: &mut EventCtx, now: Time) {
        if self.active.is_empty() {
            return;
        }
        let mut batch = GeomBatch::new();
        self.active.retain(|effect| effect.update(now, &mut batch));
        self.draw_current = ctx.upload(batch);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_current);
    }
}

impl Effect {
    fn update(&self, now: Time, batch: &mut GeomBatch) -> bool {
        let pct = (now - self.start) / (self.end - self.start);
        if pct > 1.0 {
            return false;
        }
        let scale = self.lerp_scale.0 + pct * (self.lerp_scale.1 - self.lerp_scale.0);
        batch.append(self.orig.clone().scale(scale).centered_on(self.center));
        true
    }
}

pub struct SnowEffect {
    rng: XorShiftRng,
    flakes: Vec<Snowflake>,

    draw: Drawable,
}

struct Snowflake {
    start: Time,
    top_left: Pt2D,
}

impl SnowEffect {
    pub fn new(ctx: &mut EventCtx) -> SnowEffect {
        let mut snow = SnowEffect {
            rng: XorShiftRng::seed_from_u64(42),
            flakes: Vec::new(),
            draw: Drawable::empty(ctx),
        };

        let now = Time::START_OF_DAY;
        for _ in 0..60 {
            snow.spawn_new(ctx, now);
        }
        snow.event(ctx, now);

        snow
    }

    fn spawn_new(&mut self, ctx: &EventCtx, now: Time) {
        let top_left = Pt2D::new(
            self.rng.gen_range(0.0, ctx.canvas.window_width),
            self.rng.gen_range(0.0, ctx.canvas.window_height),
        );
        self.flakes.push(Snowflake {
            start: now,
            top_left,
        });
    }

    pub fn event(&mut self, ctx: &mut EventCtx, now: Time) {
        let shape = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(50.0)).to_polygon();

        let lifetime = Duration::seconds(5.0);
        let lerp_scale = (1.0, 0.1);

        let mut batch = GeomBatch::new();
        self.flakes.retain(|flake| {
            let pct = (now - flake.start) / lifetime;
            if pct > 1.0 {
                false
            } else {
                let scale = lerp_scale.0 + pct * (lerp_scale.1 - lerp_scale.0);
                batch.push(
                    Color::WHITE.alpha(0.5),
                    shape
                        .scale(scale)
                        .translate(flake.top_left.x(), flake.top_left.y()),
                );
                true
            }
        });
        self.draw = ctx.upload(batch);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork_screenspace();
        g.redraw(&self.draw);
        g.unfork();
    }
}
