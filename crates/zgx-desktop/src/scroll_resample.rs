//! Display-paced scrolling.
//!
//! Trackpads report motion on their own clock, out of phase with the display:
//! applying each event on arrival moves content 0, 1 or 2 events' worth per
//! refresh, which reads as judder even at full frame rate. Continuous (pixel)
//! input is instead resampled: each refresh applies the motion up to one input
//! interval before the refresh, interpolated between the events around it, so
//! every frame advances evenly. Discrete wheel notches have no motion curve of
//! their own; they glide to their destination as Chrome's do
//! ([`crate::wheel_glide`]).
use std::{collections::VecDeque, time::Duration, time::Instant};

/// A pause this long ends a stroke: whatever is still buffered is applied.
const STALL: Duration = Duration::from_millis(50);
/// Refreshes keep running this long after the last input, so a gesture never
/// stops and restarts the display link (a lost refresh each time) when the
/// applied motion momentarily catches up with the input.
const LINGER: Duration = Duration::from_millis(120);
/// Bounds for the sampling delay, which tracks the input interval.
const MIN_DELAY: Duration = Duration::from_millis(4);
const MAX_DELAY: Duration = Duration::from_millis(20);

/// `ZGX_SCROLL_RESAMPLE=0` applies wheel input on arrival instead.
pub(crate) fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| zgx::env::var_os("ZGX_SCROLL_RESAMPLE").is_none_or(|v| v != "0"))
}

#[derive(Default)]
pub(crate) struct ScrollResampler {
    /// Arrival time and cumulative position of each buffered event.
    points: VecDeque<(Instant, (f32, f32))>,
    /// Cumulative continuous motion received and applied.
    received: (f32, f32),
    applied: (f32, f32),
    /// Smoothed spacing between continuous events.
    interval: Option<Duration>,
    /// Wheel notches gliding in.
    wheel: crate::wheel_glide::WheelGlide,
    /// When the latest input arrived.
    last_input: Option<Instant>,
    /// Pointer position of the latest input; motion is delivered there.
    pub position: (f32, f32),
}

impl ScrollResampler {
    pub fn pending(&self) -> bool {
        self.received != self.applied || self.wheel.active()
    }
    /// A gesture is under way: refreshes should keep coming at full rate.
    pub fn active(&self, now: Instant) -> bool {
        self.pending()
            || self
                .last_input
                .is_some_and(|at| now.duration_since(at) < LINGER)
    }
    /// Buffer pixel-precise motion that arrived at `now`.
    pub fn push_precise(&mut self, now: Instant, delta: (f32, f32)) {
        let last = self.last_input.replace(now);
        match last {
            Some(last) if now.duration_since(last) < STALL => {
                let gap = now.duration_since(last).max(Duration::from_micros(500));
                self.interval = Some(match self.interval {
                    Some(interval) => interval.mul_f32(0.7) + gap.mul_f32(0.3),
                    None => gap,
                });
            }
            _ => {
                // A new stroke: the first event's motion spans one interval
                // before it, not the whole idle gap.
                let delay = self.delay();
                self.points.clear();
                self.applied = self.received;
                self.points
                    .push_back((now.checked_sub(delay).unwrap_or(now), self.received));
            }
        }
        self.received.0 += delta.0;
        self.received.1 += delta.1;
        self.points.push_back((now, self.received));
    }
    /// Queue a discrete wheel step to glide in over the next refreshes.
    pub fn push_wheel(&mut self, now: Instant, delta: (f32, f32)) {
        self.last_input = Some(now);
        self.wheel.push(delta, now);
    }
    /// Deliver everything at once (no display refreshes are coming).
    pub fn flush(&mut self) -> Option<(f32, f32)> {
        let wheel = self.wheel.finish();
        let delta = (
            self.received.0 - self.applied.0 + wheel.0,
            self.received.1 - self.applied.1 + wheel.1,
        );
        self.applied = self.received;
        self.points.clear();
        (delta != (0., 0.)).then_some(delta)
    }
    /// The motion to apply at a display refresh processed at `now`.
    pub fn sample(&mut self, now: Instant) -> Option<(f32, f32)> {
        let target = match self.points.back() {
            Some((last, total)) if now.duration_since(*last) >= STALL => *total,
            Some(_) => self.position_at(now.checked_sub(self.delay()).unwrap_or(now)),
            None => self.received,
        };
        // Never step back: a late sample must not undo motion already shown.
        let continuous = (
            advance(self.applied.0, target.0, self.received.0),
            advance(self.applied.1, target.1, self.received.1),
        );
        let mut delta = (continuous.0 - self.applied.0, continuous.1 - self.applied.1);
        self.applied = continuous;
        // Keep one point at or before the sample time for interpolation.
        let horizon = now.checked_sub(self.delay()).unwrap_or(now);
        while self.points.len() > 1 && self.points[1].0 <= horizon {
            self.points.pop_front();
        }
        let wheel = self.wheel.advance(now);
        delta = (delta.0 + wheel.0, delta.1 + wheel.1);
        (delta != (0., 0.)).then_some(delta)
    }
    fn delay(&self) -> Duration {
        self.interval
            .unwrap_or(Duration::from_millis(8))
            .clamp(MIN_DELAY, MAX_DELAY)
    }
    /// Cumulative position at `time`, linear between the events around it.
    fn position_at(&self, time: Instant) -> (f32, f32) {
        let mut previous = match self.points.front() {
            Some((at, position)) if time <= *at => return *position,
            Some(point) => *point,
            None => return self.received,
        };
        for &(at, position) in self.points.iter().skip(1) {
            if time <= at {
                let span = at.duration_since(previous.0).as_secs_f32();
                let t = if span > 0. {
                    time.duration_since(previous.0).as_secs_f32() / span
                } else {
                    1.
                };
                return (
                    previous.1.0 + (position.0 - previous.1.0) * t,
                    previous.1.1 + (position.1 - previous.1.1) * t,
                );
            }
            previous = (at, position);
        }
        previous.1
    }
}

/// Move from `applied` towards `target` without passing `received` or
/// reversing the direction of travel.
fn advance(applied: f32, target: f32, received: f32) -> f32 {
    if received >= applied {
        target.clamp(applied, received)
    } else {
        target.clamp(received, applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MS: Duration = Duration::from_millis(1);

    /// 120 Hz input sampled by a 120 Hz display half a period out of phase:
    /// applying events on arrival alternates 0/2-event frames; resampled
    /// frames each advance by one event's worth.
    #[test]
    fn out_of_phase_input_advances_evenly() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        let step = Duration::from_micros(8333);
        let mut deltas = Vec::new();
        for i in 0..60u32 {
            // Two events land in some refresh intervals and none in others.
            let jitter = if i % 3 == 0 { 3 * MS } else { Duration::ZERO };
            resampler.push_precise(start + step * i + jitter, (0., 10.));
            let refresh = start + step * i + step / 2 + 2 * MS;
            if let Some((_, dy)) = resampler.sample(refresh) {
                deltas.push(dy);
            }
        }
        let steady = &deltas[4..deltas.len() - 4];
        for dy in steady {
            assert!((dy - 10.).abs() < 4.5, "{deltas:?}");
        }
    }

    #[test]
    fn a_stalled_stroke_applies_everything() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        resampler.push_precise(start, (0., 12.));
        resampler.push_precise(start + 8 * MS, (0., 12.));
        let first = resampler.sample(start + 9 * MS).unwrap_or_default().1;
        let rest = resampler.sample(start + 200 * MS).unwrap_or_default().1;
        assert!((first + rest - 24.).abs() < 1e-3);
        assert!(!resampler.pending());
    }

    /// A notch glides as Chrome's (and gpui Zeron's) does: ease-in-out
    /// over 200 ms, 24 refreshes at 120 Hz, in whole pixels.
    #[test]
    fn wheel_notches_glide_in_and_out_over_200_ms() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        resampler.push_wheel(start, (0., 40.));
        let mut total = 0.;
        let mut steps = Vec::new();
        let mut last = 0;
        for i in 1..=40u32 {
            if let Some((_, dy)) = resampler.sample(start + Duration::from_micros(8333) * i) {
                total += dy;
                steps.push(dy);
                last = i;
            }
        }
        assert!((total - 40.).abs() < 1e-3);
        assert!((23..=25).contains(&last), "{last} refreshes: {steps:?}");
        let peak = steps.iter().cloned().fold(0., f32::max);
        assert!(
            steps[0] < peak && *steps.last().unwrap() < peak,
            "{steps:?}"
        );
        assert!(steps.iter().all(|step| step.fract() == 0.), "{steps:?}");
        assert!(!resampler.pending());
    }

    /// Notches in a spin keep the glide going, and the whole spin lands.
    #[test]
    fn a_spin_is_one_glide_that_lands() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        let mut total = 0.;
        let mut idle = 0;
        for i in 1..=80u32 {
            let now = start + Duration::from_micros(8333) * i;
            // (A notch arrives between refreshes.)
            if i % 4 == 1 && i < 30 {
                resampler.push_wheel(now - Duration::from_millis(4), (0., 40.));
            }
            match resampler.sample(now) {
                Some((_, dy)) => total += dy,
                // (Its first refreshes ease in under a pixel.)
                None if total > 0. && i < 30 => idle += 1,
                None => {}
            }
        }
        assert_eq!(total, 320.);
        assert_eq!(idle, 0, "a refresh stood still mid-spin");
        assert!(!resampler.pending());
    }

    /// Eased notches applied far down long content land exactly on their
    /// destination, as f32 sums of fractional steps would not.
    #[test]
    fn wheel_notches_land_exactly_far_down() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        let mut offset = 79_952_f32;
        for notch in 0..5u32 {
            resampler.push_wheel(start + Duration::from_millis(25) * notch, (0., 56.));
        }
        for i in 1..=60u32 {
            if let Some((_, dy)) = resampler.sample(start + Duration::from_micros(8333) * i) {
                offset += dy;
            }
        }
        assert_eq!(offset, 79_952. + 280.);
        assert!(!resampler.pending());
    }

    #[test]
    fn flush_delivers_all_buffered_motion() {
        let start = Instant::now();
        let mut resampler = ScrollResampler::default();
        resampler.push_precise(start, (3., 5.));
        resampler.push_wheel(start, (0., 28.));
        assert_eq!(resampler.flush(), Some((3., 33.)));
        assert!(!resampler.pending());
    }
}
