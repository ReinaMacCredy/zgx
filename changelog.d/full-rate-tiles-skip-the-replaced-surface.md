---
section: fixed
---
- **A full-rate frame on macOS never draws into the surface the window still shows** (zgx-gpu): at the display's full rate (an animation, or input over a slower one) each frame drew into the surface the frame before had just replaced, which the window server could still be compositing, so a frame now and then showed half drawn: what lay under a modal without the modal, or a patchwork of old and new tiles. Frames take the third surface kept for full rate instead.
