---
section: fixed
---
- **An ellipsized text's carets skip its ellipsis** (zgx-gpu): a text cut short with `…` had its carets, its line carets and its carets by offset read from the source by the ellipsis's own bytes, which run past a short source's end or into one of its characters; accessibility reads every text's carets each frame, so a short ellipsized label (a tab's title, a chat's name) panicked with "end byte index 3 is out of bounds for string of length 2".
