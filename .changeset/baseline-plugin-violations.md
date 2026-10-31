---
"@biomejs/biome": minor
---

The lint baseline now records violations reported by plugins. Each plugin's violations are counted under `lint/plugin/<name>`, the name a suppression comment uses for it, and `--baseline-rule=plugin/<name>` records one plugin's violations. Serialized diagnostics carry the plugin's name in a new `subcategory` field.
