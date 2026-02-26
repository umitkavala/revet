---
sidebar_position: 4
---

# Language Parsers

Revet uses [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) for fast, accurate AST parsing. Each language parser builds nodes and edges in the code dependency graph.

## Supported languages

| Language | Extensions | Graph features |
|----------|-----------|----------------|
| Python | `.py` | Functions, classes, decorators, async, nested scopes |
| TypeScript / JS | `.ts`, `.tsx`, `.js`, `.jsx` | Classes, interfaces, generics, arrow functions, enums |
| Rust | `.rs` | Functions, structs, enums, traits, impl blocks |
| Go | `.go` | Functions, methods, structs, interfaces, goroutines |
| Java | `.java` | Classes, interfaces, records, enums, nested classes |
| C# | `.cs` | Classes, interfaces, records, structs, attributes, generics |
| Kotlin | `.kt`, `.kts` | Classes, objects, data classes, annotations, sealed classes |
| Ruby | `.rb`, `.rake`, `.gemspec` | Classes, modules, mixins, attr_accessors |
| PHP | `.php` | Classes, traits, enums, namespaces, attributes |
| Swift | `.swift` | Classes, structs, protocols, extensions, enums |
| C / C++ | `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx` | Functions, structs, C++ classes with inheritance, `#include` imports, macros, call graph |

## What parsers produce

Each parser creates:

- **`File` node** — one per source file, with language metadata
- **`Function` / `Method` nodes** — with parameter lists and line numbers
- **`Class` / `Struct` / `Trait` nodes** — with inheritance relationships
- **`Import` nodes** — with module specifiers for cross-file resolution
- **`Contains` edges** — file → symbol, class → method
- **`Imports` edges** — file → imported file (resolved cross-file)
- **`Calls` edges** — caller → callee (resolved cross-file)
- **`Inherits` edges** — subclass → superclass

## Incremental parsing

Parsed file graphs are cached under `.revet-cache/files/` keyed by content hash. On subsequent runs, only changed files are re-parsed by tree-sitter. Second runs on unchanged codebases are near-instant.

## Adding a language

See [Contributing](contributing) for the step-by-step guide to adding a new language parser.
