---
"@biomejs/biome": patch
---

Fixed a crash of [`noDocumentCookie`](https://biomejs.dev/linter/rules/no-document-cookie/) when a variable is initialized with itself, or variables are initialized with each other, and a property of one of them is assigned. Biome no longer aborts with a stack overflow on code such as:

```js
const a = a;
a.b = 1;
```
