//! A wheel notch's glide, as Chrome animates one (`cc::ScrollOffsetAnimationCurve`
//! for `kMouseWheel`, which it runs on macOS while `NSScrollAnimationEnabled`
//! is on, as it is by default): ease-in-out, cubic-bezier(0.42, 0, 0.58, 1),
//! over 200 ms, shortening to 100 ms for long moves. A notch in flight moves
//! the target on and starts a new leg at the current speed, so a spin is one
//! continuous glide. A notch against the glide drops what was left on its
//! axis and turns from rest. Slices are whole pixels, the rest at the end.
use std::time::Instant;

/// Chrome's easing control points (`kEaseInOutControlPoints`).
const EASE_IN_OUT: [f64; 4] = [0.42, 0.0, 0.58, 1.0];
/// Chrome's `kEpsilon`, 0.01 px, for distances and speeds.
const EPSILON: f64 = 0.01;

#[derive(Default)]
pub(crate) struct WheelGlide {
    /// Pixels not yet delivered, per axis.
    pending: (f32, f32),
    /// The leg in flight; `None` while idle.
    segment: Option<Segment>,
}

impl WheelGlide {
    pub fn active(&self) -> bool {
        self.segment.is_some()
    }

    /// Queue a notch of `delta` pixels arriving at `now`.
    pub fn push(&mut self, delta: (f32, f32), now: Instant) {
        let Some(segment) = self.segment.take() else {
            self.pending = delta;
            self.segment = Some(Segment::from_rest(delta, now));
            return;
        };
        let left = segment.remaining(now);
        let against = |heading: f32, by: f32| heading * by < 0.;
        if !against(segment.delta.0, delta.0) && !against(segment.delta.1, delta.1) {
            self.pending = (self.pending.0 + delta.0, self.pending.1 + delta.1);
            let to = (left.0 + delta.0, left.1 + delta.1);
            self.segment = Some(segment.retarget(to, now, duration(to)));
            return;
        }
        let axis = |pending: &mut f32, heading: f32, left: f32, by: f32| {
            if against(heading, by) {
                *pending = by;
                by
            } else {
                *pending += by;
                left + by
            }
        };
        let to = (
            axis(&mut self.pending.0, segment.delta.0, left.0, delta.0),
            axis(&mut self.pending.1, segment.delta.1, left.1, delta.1),
        );
        self.segment = Some(Segment::from_rest(to, now));
    }

    /// The whole pixels due at `now`, and all that is left once the leg is
    /// over.
    pub fn advance(&mut self, now: Instant) -> (f32, f32) {
        let Some(segment) = &self.segment else {
            return (0., 0.);
        };
        if segment.time(now) >= 1. {
            return self.finish();
        }
        let left = segment.remaining(now);
        let step = |pending: &mut f32, left: f32| {
            let due = (*pending - left).round();
            let due = if pending.abs() <= 1. {
                0.
            } else {
                due.clamp(-pending.abs(), pending.abs())
            };
            *pending -= due;
            due
        };
        (
            step(&mut self.pending.0, left.0),
            step(&mut self.pending.1, left.1),
        )
    }

    /// Everything still pending, at once.
    pub fn finish(&mut self) -> (f32, f32) {
        let rest = self.pending;
        self.pending = (0., 0.);
        self.segment = None;
        rest
    }
}

/// How long Chrome takes to move `delta` pixels from rest (`kInverseDelta`):
/// 12 frames at 60 Hz (200 ms) up to 120 px, down to 6 (100 ms) at 480 px.
fn duration(delta: (f32, f32)) -> f64 {
    let px = f64::from(major(delta)).abs();
    let frames = 12. - (px - 120.) * (12. - 6.) / (480. - 120.);
    frames.clamp(6., 12.) / 60.
}

/// The larger axis, with its sign (`MaximumDimension`).
fn major(delta: (f32, f32)) -> f32 {
    if delta.0.abs() > delta.1.abs() {
        delta.0
    } else {
        delta.1
    }
}

/// One leg: `delta` pixels from `start` over `duration` seconds.
struct Segment {
    delta: (f32, f32),
    start: Instant,
    duration: f64,
    ease: Bezier,
}

impl Segment {
    /// `slope`, when given, starts the leg at that many times its average
    /// speed (`EaseInOutWithInitialSlope`).
    fn new(delta: (f32, f32), start: Instant, duration: f64, slope: Option<f64>) -> Self {
        let [x1, y1, x2, y2] = EASE_IN_OUT;
        let y1 = slope.map_or(y1, |slope| x1 * slope.clamp(-1000., 1000.));
        Self {
            delta,
            start,
            duration,
            ease: Bezier::new(x1, y1, x2, y2),
        }
    }

    fn from_rest(delta: (f32, f32), start: Instant) -> Self {
        Self::new(delta, start, duration(delta), None)
    }

    /// How far through its time the leg is at `now`, 0 to 1.
    fn time(&self, now: Instant) -> f64 {
        if self.duration <= 0. {
            return 1.;
        }
        let elapsed = now.saturating_duration_since(self.start).as_secs_f64();
        (elapsed / self.duration).min(1.)
    }

    /// What the leg has still to travel after `now`.
    fn remaining(&self, now: Instant) -> (f32, f32) {
        let left = 1. - self.ease.value(self.time(now));
        (
            (f64::from(self.delta.0) * left) as f32,
            (f64::from(self.delta.1) * left) as f32,
        )
    }

    /// Speed along the larger axis at `now`, pixels a second.
    fn velocity(&self, now: Instant) -> f64 {
        if self.duration <= 0. {
            return 0.;
        }
        self.ease.slope(self.time(now)) * f64::from(major(self.delta)) / self.duration
    }

    /// Chrome's `UpdateTarget`: a new leg to `delta` from here, starting at
    /// this one's speed, cut short when that speed would get there sooner.
    fn retarget(&self, delta: (f32, f32), now: Instant, duration: f64) -> Self {
        let distance = f64::from(major(delta));
        if distance.abs() < EPSILON {
            return Self::new(delta, now, 0., None);
        }
        let velocity = self.velocity(now);
        let bound = (velocity.abs() >= EPSILON)
            .then(|| distance / velocity * 2.5)
            .filter(|bound| *bound >= 0.);
        let duration = bound.map_or(duration, |bound| duration.min(bound));
        if duration < 0.01 {
            return Self::new(delta, now, 0., None);
        }
        let slope = velocity * duration / distance;
        Self::new(delta, now, duration, Some(slope))
    }
}

/// Chromium's `gfx::CubicBezier`: the unit cubic from (0, 0) to (1, 1),
/// read as y of x.
struct Bezier {
    ax: f64,
    bx: f64,
    cx: f64,
    ay: f64,
    by: f64,
    cy: f64,
}

impl Bezier {
    const EPSILON: f64 = 1e-7;

    fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let (cx, cy) = (3. * x1, 3. * y1);
        let (bx, by) = (3. * (x2 - x1) - cx, 3. * (y2 - y1) - cy);
        Self {
            ax: 1. - cx - bx,
            bx,
            cx,
            ay: 1. - cy - by,
            by,
            cy,
        }
    }

    fn x(&self, t: f64) -> f64 {
        ((self.ax * t + self.bx) * t + self.cx) * t
    }

    fn y(&self, t: f64) -> f64 {
        ((self.ay * t + self.by) * t + self.cy) * t
    }

    fn dx(&self, t: f64) -> f64 {
        (3. * self.ax * t + 2. * self.bx) * t + self.cx
    }

    fn dy(&self, t: f64) -> f64 {
        (3. * self.ay * t + 2. * self.by) * t + self.cy
    }

    /// The parameter where the curve reaches `x` (`SolveCurveX`).
    fn solve(&self, x: f64) -> f64 {
        let mut t = x;
        for _ in 0..4 {
            let error = self.x(t) - x;
            if error.abs() < Self::EPSILON {
                return t;
            }
            let slope = self.dx(t);
            if slope.abs() < Self::EPSILON {
                break;
            }
            t -= error / slope;
        }
        let (mut low, mut high) = (0., 1.);
        t = x;
        for _ in 0..64 {
            let at = self.x(t);
            if (at - x).abs() < Self::EPSILON {
                break;
            }
            if at < x {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) / 2.;
        }
        t
    }

    fn value(&self, x: f64) -> f64 {
        self.y(self.solve(x.clamp(0., 1.)))
    }

    fn slope(&self, x: f64) -> f64 {
        let t = self.solve(x.clamp(0., 1.));
        let (dx, dy) = (self.dx(t), self.dy(t));
        if dx == 0. && dy == 0. { 0. } else { dy / dx }
    }
}
