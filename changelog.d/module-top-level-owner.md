---
section: fixed
---
- **A module's top level outlives whoever first required it** (zgx-script): a module required while a component is built or an effect runs keeps its computeds and effects once that component goes, instead of leaving the module's cached state frozen.
