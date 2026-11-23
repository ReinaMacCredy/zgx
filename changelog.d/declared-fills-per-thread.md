---
section: fixed
---
- **A renderer compiles only the fills declared on its own thread** (zgx-gpu): the list `ui.shader` declares fills to (`warm_fill_shader`) was one for the whole process, while the scripts' shaders and the renderers are each a thread's own. In a process with apps on several threads (a test binary: every test in parallel), each renderer compiled every other thread's fills, hundreds of pipelines it never drew, and a small GPU (a macOS VM's) ran out of memory. The list is a thread's own now; an app's script and renderers share a thread, so it compiles what it did.
