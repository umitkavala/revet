---
sidebar_position: 5
---

# React Hooks

Off by default (`modules.react = true` to enable). Detects Rules of Hooks violations and common React anti-patterns. Prefix: `HOOKS-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `HOOKS-001` | Error | Hook called inside a condition or loop |
| `HOOKS-002` | Warning | `useEffect` without dependency array |
| `HOOKS-003` | Warning | Direct DOM manipulation in component |
| `HOOKS-004` | Warning | Missing `key` prop in `.map()` |
| `HOOKS-005` | Warning | `dangerouslySetInnerHTML` usage |
| `HOOKS-006` | Info | Inline arrow function in event handler |
| `HOOKS-007` | Info | Empty dependency array `[]` in `useEffect` |

```tsx
// Bad — hook in condition (HOOKS-001)
if (condition) {
  const [value, setValue] = useState(null);
}

// Bad — missing deps (HOOKS-002)
useEffect(() => {
  fetchData();
});

// Good
useEffect(() => {
  fetchData();
}, [userId]);
```
