//! Custom shaders' pipelines, compiled off the frame: on one thread, a
//! shader at a time, as a script declares them (`warm_fill_shader`) or a
//! frame first meets them. A window's frame that meets one still compiling
//! draws without it and goes on; the thread wakes the host when it is done
//! (`zgx::image::wake_waiting`), which draws again. A compile takes 70 ms
//! with Metal's shader cache warm, 360 ms the first time after a build: a
//! frame waiting for it froze the window that long.

use std::sync::{Arc, Mutex, OnceLock, Weak};

/// What a compile makes, once it has: `get` is `None` until then.
pub(crate) type Slot<T> = Arc<OnceLock<T>>;

type Job = Box<dyn FnOnce() + Send>;

/// Run `make` on the compile thread; its result lands in the slot returned,
/// and the host is woken for any frame that went without it.
pub(crate) fn spawn<T: Send + Sync + 'static>(
    make: impl FnOnce() -> T + Send + 'static,
) -> Slot<T> {
    static WORKER: OnceLock<Mutex<std::sync::mpsc::Sender<Job>>> = OnceLock::new();
    let slot: Slot<T> = Arc::default();
    let filled = slot.clone();
    let job: Job = Box::new(move || {
        let _ = filled.set(make());
        zgx::image::wake_waiting();
    });
    let sender = WORKER.get_or_init(|| {
        let (send, receive) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("zgx shader compile".into())
            .spawn(move || {
                for job in receive {
                    job();
                }
            })
            .expect("a shader compiling thread");
        Mutex::new(send)
    });
    let _ = sender.lock().unwrap_or_else(|p| p.into_inner()).send(job);
    slot
}

/// `spawn`, or with `here` (a renderer that waits for its shaders anyway:
/// offscreen, or told to, `GpuRenderer::waits_for_shaders`) `make` run on
/// this thread, its slot filled when this returns. Such a renderer would
/// only block for the compile thread; meanwhile both threads allocated, and
/// how their allocations interleaved moved what its frames counted (the
/// gate's fx cells, run to run, by up to 0.46%).
pub(crate) fn start<T: Send + Sync + 'static>(
    here: bool,
    make: impl FnOnce() -> T + Send + 'static,
) -> Slot<T> {
    if here {
        return Arc::new(OnceLock::from(make()));
    }
    spawn(make)
}

/// A compile started or done, in a renderer's cache of pipelines by shader.
pub(crate) enum Compiled<T> {
    Pending(Slot<Option<T>>),
    /// `None`: it did not compile (the box fills with its colour).
    Done(Option<T>),
}

impl<T> Compiled<T> {
    /// Whether it can draw, moving a finished compile's pipeline in; `wait`
    /// blocks for one still compiling (an offscreen renderer, whose frames
    /// have no deadline and nothing to wake). `None` while it compiles.
    pub(crate) fn ready(&mut self, wait: bool) -> Option<bool> {
        if let Self::Pending(slot) = self {
            let compiled = match wait {
                true => slot.wait().is_some(),
                false => slot.get()?.is_some(),
            };
            // The compile thread lets go of the slot once it has filled it:
            // the pipeline moves in then (until then, `get` reads it there).
            match Arc::get_mut(slot).and_then(OnceLock::take) {
                Some(pipeline) => *self = Self::Done(pipeline),
                None => return Some(compiled),
            }
        }
        Some(self.get().is_some())
    }

    /// The pipeline, once compiled.
    pub(crate) fn get(&self) -> Option<&T> {
        match self {
            Self::Done(pipeline) => pipeline.as_ref(),
            Self::Pending(slot) => slot.get()?.as_ref(),
        }
    }
}

thread_local! {
    /// Fill shaders a script on this thread declared (`ui.shader`), for each
    /// of the thread's renderers to start compiling before anything draws
    /// them. A thread's own, as the script's shaders are (its `ui.shader`
    /// cache) and a renderer is: an app on another thread (each test of a
    /// test binary) draws with renderers of its own, and a renderer that
    /// compiled every thread's fills made hundreds of pipelines it never
    /// drew, which a small GPU ran out of memory for.
    static DECLARED: std::cell::RefCell<Vec<Weak<zgx::decoration::FillShader>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// How many `DECLARED` has had: a renderer that has seen as many has
    /// nothing new to start.
    static DECLARED_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Start compiling `shader`'s fill in every renderer of this thread, as it
/// is declared, so the frame that first draws it finds it compiled: a status
/// ring shown the moment work starts appears with it, not a frame that froze
/// for its compile. Renderer plumbing for the script host.
pub fn warm_fill_shader(shader: &Arc<zgx::decoration::FillShader>) {
    DECLARED.with_borrow_mut(|declared| {
        declared.retain(|shader| shader.strong_count() > 0);
        declared.push(Arc::downgrade(shader));
    });
    DECLARED_COUNT.set(DECLARED_COUNT.get() + 1);
}

/// Whether `shader` is one declared (`warm_fill_shader`) and still held.
#[cfg(test)]
pub(crate) fn is_declared(shader: &Arc<zgx::decoration::FillShader>) -> bool {
    DECLARED.with_borrow(|declared| {
        declared.iter().any(|held| {
            held.upgrade()
                .is_some_and(|held| Arc::ptr_eq(&held, shader))
        })
    })
}

/// How many fill shaders scripts have declared (`declared_since`).
#[inline]
pub(crate) fn declared() -> u64 {
    DECLARED_COUNT.get()
}

/// The fill shaders declared, if more were since `seen` (updated).
pub(crate) fn declared_since(seen: &mut u64) -> Option<Vec<Arc<zgx::decoration::FillShader>>> {
    let count = declared();
    if count == *seen {
        return None;
    }
    *seen = count;
    Some(DECLARED.with_borrow(|declared| declared.iter().filter_map(Weak::upgrade).collect()))
}
