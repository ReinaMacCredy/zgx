---
section: added
---
- **A page can keep the app's scripts out of its reach** (zgx, zgx-desktop, zgx-script): `WebViewOptions::isolated` (Luau `isolated = true` or a list of scripts) puts the message channel, those scripts and `evaluate_js` (`run`, `eval`) in a `WKContentWorld` of their own on macOS. They share the page's DOM and none of its globals; the page's own scripts find no `window.zgx` nor its WebKit handler, so they can't post to the app or hook what its scripts use. WebKitGTK and WebView2 keep one world: there the channel stays in the page's and the scripts don't run (`WebView::isolated`, Luau `web.isolated`, says which).
