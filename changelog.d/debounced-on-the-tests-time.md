---
section: fixed
---
- **`ui.debounced` rests on the test's time under `zgx test`** (zgx-script): its wait ran on the machine's clock while `ui.after` and `ui.every` kept the test's, so a test that waited a fixed time for a rest (a scrolled view's place written 150 ms after its offset stops) passed or failed with how late a busy machine's timers ran. It now rests in the frame its time falls in, as the other timers fire; in a window nothing changes.
