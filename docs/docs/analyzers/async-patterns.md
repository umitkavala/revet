---
sidebar_position: 6
---

# Async Patterns

Off by default (`modules.async_patterns = true` to enable). Detects async/await anti-patterns in JavaScript, TypeScript, and Python. Prefix: `ASYNC-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `ASYNC-001` | Error | Async function passed to `Promise` executor |
| `ASYNC-002` | Error | `forEach` with async callback |
| `ASYNC-003` | Warning | Unhandled `.then()` chain (no `.catch()`) |
| `ASYNC-004` | Warning | `async` in `.map()` without `Promise.all` |
| `ASYNC-005` | Warning | Async callback in `setTimeout`/`setInterval` |
| `ASYNC-006` | Warning | Floating Python coroutine (not awaited) |
| `ASYNC-007` | Info | Empty `.catch(() => {})` swallowing errors |
| `ASYNC-008` | Info | Redundant `return await` inside `async` |

```ts
// Bad — ASYNC-002
items.forEach(async (item) => {
  await process(item); // errors are lost
});

// Good
await Promise.all(items.map(item => process(item)));

// Bad — ASYNC-004
const results = items.map(async (item) => { ... });
// results is Promise<...>[], not resolved values

// Good
const results = await Promise.all(items.map(async (item) => { ... }));
```
