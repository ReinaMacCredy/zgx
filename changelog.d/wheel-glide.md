---
section: changed
---
- **A mouse wheel's notch glides as Chrome's does, and an app can turn it off** (zgx, zgx-script, zgx-desktop): ease-in-out over 200 ms, 100 ms for long moves, a notch in flight carrying the glide on at its speed, where notches closed exponentially in about 100 ms. `ui.window.smooth_scrolling:set(false)` (`Ui::set_wheel_glides`) steps each notch at once.
