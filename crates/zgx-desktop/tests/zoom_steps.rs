//! macOS, on a logged-in screen: each step of AppKit's zoom lays out at the size the window has then.
fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSWorkspace};
    use std::{cell::RefCell, rc::Rc, time::Duration};
    use zgx::compose::prelude::*;
    use zgx_desktop::{Application, WindowOptions};

    fn window_width() -> Option<f64> {
        let mtm = MainThreadMarker::new()?;
        let windows = NSApplication::sharedApplication(mtm).windows();
        windows
            .iter()
            .map(|window| window.frame().size.width)
            .next()
    }

    pub fn run() {
        if NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
            println!("zoom_steps: skipped, Reduce motion turns AppKit's animation off");
            return;
        }
        let outcome = Application::new()
            .window(WindowOptions {
                title: "zgx zoom steps".into(),
                width: 400.,
                height: 300.,
                ..Default::default()
            })
            .run(|cx| {
                let layouts = Rc::new(RefCell::new(Vec::<(f32, f64)>::new()));
                let viewport = cx.viewport.clone();
                let seen = layouts.clone();
                cx.render(column().reactive_style(move || {
                    let (width, _) = viewport.get();
                    if let Some(window) = window_width() {
                        seen.borrow_mut().push((width, window));
                    }
                    Styles::new()
                }));
                let window = cx.window.clone();
                cx.tasks.spawn(async move {
                    zgx::timer::sleep(Duration::from_millis(600)).await;
                    window.set_maximized(true);
                    zgx::timer::sleep(Duration::from_millis(1500)).await;
                    let layouts = layouts.borrow();
                    let behind = layouts
                        .iter()
                        .filter(|(page, window)| (*page as f64 - window).abs() > 1.)
                        .count();
                    let mut widths: Vec<_> = layouts.iter().map(|(page, _)| *page).collect();
                    widths.dedup();
                    println!(
                        "zoom_steps: {} layouts, {} widths, {behind} behind the window",
                        layouts.len(),
                        widths.len(),
                    );
                    // A panic here is contained and the app keeps running.
                    std::process::exit(if behind == 0 && widths.len() > 3 {
                        0
                    } else {
                        1
                    });
                });
            });
        if let Err(error) = outcome {
            println!("zoom_steps: skipped, no window here: {error}");
        }
    }
}
