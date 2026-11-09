---
section: changed
---
- **`WebViewOptions` has a new field, `isolated`** (zgx): a `WebViewOptions` written out field by field stops compiling. *Migrate:* end it with `..Default::default()`, or set `isolated: None` (the default).
