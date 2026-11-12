---
section: added
---
- **A page hands on the scrolls it can't use** (zgx, zgx-desktop, zgx-script; macOS): `WebView::set_scroll_chain(ScrollChain { up, down, left, right })`, `web:scroll_chain { … }` in Luau, names the directions in which wheel and trackpad scrolls over the page go on to the app's view under it, as a frame nested in a browser's page passes on what it has no room for. A gesture that begins over the page stays with it until the page reaches an edge the app chains past, then the rest of it, momentum included, scrolls what is under the page; one that began elsewhere stays there. By default the page keeps every scroll, as before.
