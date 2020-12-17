use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use geom::{Distance, Duration, PolyLine, Pt2D, Time};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor};

pub struct Animator {
    active: Vec<Animation>,
    draw_mapspace: Drawable,
    draw_screenspace: Option<Drawable>,
}

struct Animation {
    start: Time,
    end: Time,
    effect: Effect,
    screenspace: bool,
}

pub enum Effect {
    Scale {
        orig: GeomBatch,
        center: Pt2D,
        lerp_scale: (f64, f64),
    },
    FollowPath {
        color: Color,
        width: Distance,
        pl: PolyLine,
    },
    Flash {
        orig: GeomBatch,
        alpha_scale: (f32, f32),
        cycles: usize,
    },
}

impl Animator {
    pub fn new(ctx: &EventCtx) -> Animator {
        Animator {
            active: Vec::new(),
            draw_mapspace: Drawable::empty(ctx),
            draw_screenspace: None,
        }
    }

    /// Pass in a future value for `now` to schedule a delayed effect
    pub fn add(&mut self, now: Time, duration: Duration, effect: Effect) {
        self.active.push(Animation {
            start: now,
            end: now + duration,
            effect,
            screenspace: false,
        });
    }

    pub fn add_screenspace(&mut self, now: Time, duration: Duration, effect: Effect) {
        self.active.push(Animation {
            start: now,
            end: now + duration,
            effect,
            screenspace: true,
        });
    }

    pub fn event(&mut self, ctx: &mut EventCtx, now: Time) {
        if self.active.is_empty() {
            return;
        }
        let mut mapspace = GeomBatch::new();
        let mut screenspace = GeomBatch::new();
        self.active.retain(|anim| {
            let pct = (now - anim.start) / (anim.end - anim.start);
            if pct < 0.0 {
                // Hasn't started yet
                true
            } else if pct > 1.0 {
                false
            } else {
                if anim.screenspace {
                    anim.effect.render(pct, &mut screenspace);
                } else {
                    anim.effect.render(pct, &mut mapspace);
                }
                true
            }
        });
        self.draw_mapspace = ctx.upload(mapspace);
        if screenspace.is_empty() {
            self.draw_screenspace = None;
        } else {
            self.draw_screenspace = Some(ctx.upload(screenspace));
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_mapspace);
        if let Some(ref d) = self.draw_screenspace {
            g.fork_screenspace();
            g.redraw(d);
            g.unfork();
        }
    }

    pub fn is_done(&self) -> bool {
        self.active.is_empty()
    }
}

impl Effect {
    fn render(&self, pct: f64, batch: &mut GeomBatch) {
        match self {
            Effect::Scale {
                ref orig,
                center,
                lerp_scale,
            } => {
                let scale = lerp_scale.0 + pct * (lerp_scale.1 - lerp_scale.0);
                batch.append(orig.clone().scale(scale).centered_on(*center));
            }
            Effect::FollowPath {
                color,
                width,
                ref pl,
            } => {
                if let Ok(pl) = pl.maybe_exact_slice(Distance::ZERO, pct * pl.length()) {
                    batch.push(*color, pl.make_polygons(*width));
                }
            }
            Effect::Flash {
                ref orig,
                alpha_scale,
                cycles,
            } => {
                // -1 to 1
                let shift = (pct * (*cycles as f64) * (2.0 * std::f64::consts::PI)).sin() as f32;
                let midpt = (alpha_scale.0 + alpha_scale.1) / 2.0;
                let half_range = (alpha_scale.1 - alpha_scale.0) / 2.0;
                let alpha = midpt + shift * half_range;

                batch.append(orig.clone().color(RewriteColor::ChangeAlpha(alpha)));
            }
        }
    }
}

pub struct SnowEffect {
    rng: XorShiftRng,
    flakes: Vec<Snowflake>,

    draw: Drawable,
}

struct Snowflake {
    start: Time,
    initial_pos: Pt2D,
    fall_speed: f64,
    swoop_period: f64,
    max_swoop: f64,
}

impl Snowflake {
    fn pos(&self, time: Time) -> Pt2D {
        let arg =
            (2.0 * std::f64::consts::PI) * (time - self.start).inner_seconds() / self.swoop_period;
        let x = self.initial_pos.x() + self.max_swoop * arg.cos();
        let y = self.initial_pos.y() + self.fall_speed * (time - self.start).inner_seconds();
        Pt2D::new(x, y)
    }
}

impl SnowEffect {
    pub fn new(ctx: &mut EventCtx) -> SnowEffect {
        let mut snow = SnowEffect {
            rng: XorShiftRng::seed_from_u64(42),
            flakes: Vec::new(),
            draw: Drawable::empty(ctx),
        };

        let now = Time::START_OF_DAY;
        // TODO Amp back up after fixing slow performance in debug mode
        for _ in 0..20 {
            let initial_pos = Pt2D::new(
                snow.rng.gen_range(0.0, ctx.canvas.window_width),
                snow.rng.gen_range(0.0, ctx.canvas.window_height),
            );
            let flake = snow.spawn_new(now, initial_pos);
            snow.flakes.push(flake);
        }
        snow.event(ctx, now);

        snow
    }

    fn spawn_new(&mut self, now: Time, initial_pos: Pt2D) -> Snowflake {
        Snowflake {
            start: now,
            initial_pos,
            // Pixels per second
            // TODO It'd be neat to speed this up as time runs out
            fall_speed: self.rng.gen_range(150.0, 300.0),
            swoop_period: self.rng.gen_range(1.0, 5.0),
            // Pixels
            max_swoop: self.rng.gen_range(0.0, 50.0),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, now: Time) {
        let shape = GeomBatch::load_svg(ctx, "system/assets/map/snowflake.svg").scale(0.1);

        let mut batch = GeomBatch::new();
        let prev_flakes = std::mem::replace(&mut self.flakes, Vec::new());
        let mut new_flakes = Vec::new();
        for flake in prev_flakes {
            let pt = flake.pos(now);
            if pt.y() > ctx.canvas.window_height {
                let initial_pos = Pt2D::new(self.rng.gen_range(0.0, ctx.canvas.window_width), 0.0);
                new_flakes.push(self.spawn_new(now, initial_pos));
            } else {
                batch.append(shape.clone().translate(pt.x(), pt.y()));
                new_flakes.push(flake);
            }
        }
        self.flakes = new_flakes;
        self.draw = ctx.upload(batch);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork_screenspace();
        g.redraw(&self.draw);
        g.unfork();
    }
}
