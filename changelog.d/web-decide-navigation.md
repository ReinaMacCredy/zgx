---
section: added
---
- **A script decides a page's navigations** (zgx-script): `web:decide_navigation(fn)` asks `fn({ url, main_frame, kind, source_url })` before each navigation starts, with what started it (`"link"`, `"form"`, `"back_forward"`, `"reload"`, `"form_resubmission"`, `"other"`); `false` refuses it. The app's guard still goes first.
