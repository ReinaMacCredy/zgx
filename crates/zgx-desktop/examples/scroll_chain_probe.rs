//! A page in a scroll view hands on the scrolls it can't use (macOS,
//! `WebView::set_scroll_chain`), by two-finger scroll gestures sent to this
//! app's own window (CGEvent scrolls with their phases; the user's trackpad
//! and other apps hear nothing). A 400 × 240 page sits 100 points down a
//! scroll view taller than the window. In turn, each gesture toward the
//! bottom over the page:
//!
//! - `kept`: a page that fits, no chain: the page keeps the gesture and the
//!   view stays (what a page did before the chain);
//! - `fits`: the same page chaining every way: the view scrolls;
//! - `inside`: a page four times its box, chaining as its edges say (it
//!   posts them as it scrolls): the page scrolls, the view stays;
//! - `edge`: a longer gesture: the page scrolls to its bottom, then the
//!   view scrolls on;
//! - `back`: the view back at its top, a gesture toward the top: the page
//!   (left mid-gesture by the last one) scrolls back up.
//!
//! It prints `STEP <name>` as each gesture starts, `VIEW <offset>` and
//! `PAGE <scrollY>` after each, and `CHAIN_DONE` at the end.
//! `scripts/macos_scroll_chain_probe.py` runs it and checks the lines.
use std::time::Duration;
use zgx::{
    compose::prelude::*,
    web_view::{ScrollChain, WebView, WebViewEvent, WebViewOptions},
};
use zgx_desktop::{Application, WindowOptions};

/// The gestures' place in the window (points): over the page.
const OVER_PAGE: (f32, f32) = (200., 220.);
const SHORT: &str = r#"<!doctype html><html><body style="margin:0;background:#3a86ff;height:100%">fits</body></html>"#;
const TALL: &str = r#"<!doctype html><html><body style="margin:0"><div style="height:960px;background:linear-gradient(#ff4d6d,#3ddc97)">tall</div>
<script>(function(){var l=-1,e=function(){var s=document.scrollingElement,y=s.scrollTop,v=(y<=0?1:0)|(y+s.clientHeight>=s.scrollHeight-1?2:0);if(v===l)return;l=v;window.zgx.postMessage({edges:v});};addEventListener("scroll",e,{passive:true});addEventListener("load",e);e();})();</script>
</body></html>"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new()
        .window(WindowOptions {
            title: "Scroll chain".into(),
            width: 600.,
            height: 500.,
            ..Default::default()
        })
        .run(move |cx| {
            let offset = cx.ui.signal(0_f32);
            let web = WebView::new(&cx.ui.runtime, WebViewOptions::default());
            web.load_html(SHORT, None);
            let chained = web.clone();
            let edges = web.on_event(move |event| {
                if let WebViewEvent::Message { body, .. } = event
                    && let Some(v) = serde_json::from_str::<serde_json::Value>(body)
                        .ok()
                        .and_then(|value| value.get("edges")?.as_u64())
                {
                    chained.set_scroll_chain(ScrollChain {
                        up: v & 1 != 0,
                        down: v & 2 != 0,
                        left: true,
                        right: true,
                    });
                }
            });
            cx.render(
                scroll(offset.clone())
                    .w_full()
                    .h_full()
                    .child(div().w_full().h(100.).bg(rgba(0x80808040)))
                    .child(web_view(&web).w(400.).h(240.))
                    .child(div().w_full().h(1200.).bg(rgba(0x40608040))),
            );
            let window = cx.window.clone();
            cx.tasks.spawn(async move {
                let _edges = edges;
                let pause = |ms| zgx::timer::sleep(Duration::from_millis(ms));
                pause(2500).await;
                let steps: [(&str, f64, usize); 5] = [
                    ("kept", -14., 15),
                    ("fits", -14., 15),
                    ("inside", -14., 12),
                    ("edge", -40., 30),
                    ("back", 14., 12),
                ];
                for (name, delta, count) in steps {
                    match name {
                        "fits" => web.set_scroll_chain(ScrollChain::ALL),
                        "back" => {
                            offset.set(0.);
                            pause(300).await;
                        }
                        "inside" => {
                            web.set_scroll_chain(ScrollChain::default());
                            offset.set(0.);
                            web.load_html(TALL, None);
                            pause(1500).await;
                        }
                        _ => {}
                    }
                    println!("STEP {name}");
                    if !trackpad::gesture(OVER_PAGE, delta, count).await {
                        window.close();
                        return;
                    }
                    pause(900).await;
                    println!("VIEW {}", offset.get());
                    let (tx, rx) = std::sync::mpsc::channel();
                    web.evaluate_js("document.scrollingElement.scrollTop", move |result| {
                        let _ = tx.send(format!("{result:?}"));
                    });
                    pause(300).await;
                    println!("PAGE {}", rx.try_recv().unwrap_or_default());
                }
                println!("CHAIN_DONE");
                window.close();
            });
        })
}

#[cfg(target_os = "macos")]
mod trackpad {
    use objc2::{MainThreadMarker, rc::Retained};
    use objc2_app_kit::{NSApplication, NSEvent, NSScreen, NSWindow};
    use objc2_core_graphics::{CGEvent, CGEventField, CGScrollEventUnit};
    use std::time::Duration;

    /// The trackpad's scroll phases (`kCGScrollPhase…`).
    const MAY_BEGIN: i64 = 128;
    const BEGAN: i64 = 1;
    const CHANGED: i64 = 2;
    const ENDED: i64 = 4;

    /// A two-finger gesture at `at` (the window's points): `count` steps of
    /// `delta` points each (negative: toward the bottom), 16 ms apart.
    /// Whether the window was there to take it.
    pub async fn gesture(at: (f32, f32), delta: f64, count: usize) -> bool {
        let mut phases = vec![(MAY_BEGIN, 0.), (BEGAN, delta)];
        phases.extend(std::iter::repeat_n((CHANGED, delta), count));
        phases.push((ENDED, 0.));
        for (phase, delta) in phases {
            let Some((window, event)) = scroll(at, phase, delta) else {
                return false;
            };
            window.sendEvent(&event);
            zgx::timer::sleep(Duration::from_millis(16)).await;
        }
        true
    }

    fn scroll(
        at: (f32, f32),
        phase: i64,
        delta: f64,
    ) -> Option<(Retained<NSWindow>, Retained<NSEvent>)> {
        let mtm = MainThreadMarker::new().unwrap();
        // (Not the key window: an app started from a terminal may not be let in front.)
        let Some(window) = NSApplication::sharedApplication(mtm)
            .windows()
            .firstObject()
        else {
            println!("NO_KEY_WINDOW");
            return None;
        };
        // A scroll made from a CGEvent has no window: its location is where
        // it is on the screen, put where, read as the window's, it is `at`.
        let content = window.contentRectForFrameRect(window.frame());
        let main_height = NSScreen::screens(mtm)
            .firstObject()
            .map_or(0., |screen| screen.frame().size.height);
        let x = f64::from(at.0);
        let y = main_height - (content.size.height - f64::from(at.1));
        let event = CGEvent::new_scroll_wheel_event2(
            None,
            CGScrollEventUnit::Pixel,
            1,
            delta as i32,
            0,
            0,
        )?;
        let set = |field, value| CGEvent::set_integer_value_field(Some(&event), field, value);
        set(CGEventField::ScrollWheelEventIsContinuous, 1);
        set(CGEventField::ScrollWheelEventScrollPhase, phase);
        set(CGEventField::ScrollWheelEventMomentumPhase, 0);
        CGEvent::set_location(Some(&event), objc2_core_foundation::CGPoint::new(x, y));
        Some((window, NSEvent::eventWithCGEvent(&event)?))
    }
}
#[cfg(not(target_os = "macos"))]
mod trackpad {
    pub async fn gesture(_: (f32, f32), _: f64, _: usize) -> bool {
        false
    }
}
