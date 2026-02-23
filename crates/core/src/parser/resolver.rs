//! Cross-file import resolution: connects Import nodes to their target symbols.
//!
//! After all files are parsed and merged into a single [`CodeGraph`], this pass
//! walks every [`UnresolvedImport`] record, resolves the module specifier to an
//! absolute file path that exists in the graph, then adds:
//!
//! - A file-level [`EdgeKind::Imports`] edge from the importing file node to the
//!   target file node.
//! - A per-symbol [`EdgeKind::References`] edge from the [`NodeKind::Import`] node
//!   to each resolved target symbol node.
//! - A [`EdgeKind::Calls`] edge for every [`UnresolvedCall`] whose callee can be
//!   matched to a symbol in the resolved target file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::graph::{CodeGraph, Edge, EdgeKind, EdgeMetadata, NodeData, NodeId, NodeKind};

use super::{UnresolvedCall, UnresolvedImport};

/// Resolves collected import/call records into concrete cross-file graph edges.
pub struct CrossFileResolver<'a> {
    root: &'a Path,
}

impl<'a> CrossFileResolver<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// Run resolution over the merged graph.
    ///
    /// Mutates `graph` by adding cross-file edges and setting
    /// `NodeData::Import.resolved_path` on resolved import nodes.
    pub fn resolve(
        &self,
        graph: &mut CodeGraph,
        imports: Vec<UnresolvedImport>,
        calls: Vec<UnresolvedCall>,
    ) {
        // Build file-path → file NodeId index
        let file_index: HashMap<PathBuf, NodeId> = graph
            .nodes()
            .filter_map(|(id, node)| {
                if matches!(node.kind(), NodeKind::File) {
                    Some((node.file_path().clone(), id))
                } else {
                    None
                }
            })
            .collect();

        // Build (file_path, symbol_name) → NodeId index for all non-File/Import nodes
        let symbol_index: HashMap<(PathBuf, String), NodeId> = graph
            .nodes()
            .filter_map(|(id, node)| {
                if !matches!(node.kind(), NodeKind::File | NodeKind::Import) {
                    Some(((node.file_path().clone(), node.name().to_string()), id))
                } else {
                    None
                }
            })
            .collect();

        let mut edges_to_add: Vec<(NodeId, NodeId, Edge)> = Vec::new();
        let mut import_resolutions: Vec<(NodeId, PathBuf)> = Vec::new();

        // ── Resolve imports ──────────────────────────────────────────────────
        for imp in imports {
            let Some(target_path) =
                self.resolve_module(&imp.module_specifier, &imp.importing_file, &file_index)
            else {
                continue; // external / unresolvable module
            };

            let Some(&target_file_id) = file_index.get(&target_path) else {
                continue;
            };

            import_resolutions.push((imp.import_node_id, target_path.clone()));

            // File-level Imports edge
            edges_to_add.push((
                imp.importing_file_node_id,
                target_file_id,
                Edge::with_metadata(
                    EdgeKind::Imports,
                    EdgeMetadata::Import {
                        alias: None,
                        is_wildcard: imp.is_wildcard,
                    },
                ),
            ));

            if imp.is_wildcard || imp.imported_names.is_empty() {
                continue;
            }

            // Per-symbol References edges
            for name in &imp.imported_names {
                let key = (target_path.clone(), name.clone());
                if let Some(&target_sym_id) = symbol_index.get(&key) {
                    edges_to_add.push((
                        imp.import_node_id,
                        target_sym_id,
                        Edge::new(EdgeKind::References),
                    ));
                }
            }
        }

        // ── Resolve cross-file calls ─────────────────────────────────────────
        for call in calls {
            let Some(target_path) =
                self.resolve_module(&call.module_specifier, &call.importing_file, &file_index)
            else {
                continue;
            };

            let key = (target_path, call.callee_name.clone());
            if let Some(&callee_id) = symbol_index.get(&key) {
                edges_to_add.push((
                    call.caller_node_id,
                    callee_id,
                    Edge::with_metadata(
                        EdgeKind::Calls,
                        EdgeMetadata::Call {
                            line: call.call_line,
                            is_direct: true,
                        },
                    ),
                ));
            }
        }

        // Apply edges (must not borrow graph mutably above)
        for (from, to, edge) in edges_to_add {
            graph.add_edge(from, to, edge);
        }

        // Stamp resolved_path on Import nodes
        for (import_node_id, resolved_path) in import_resolutions {
            if let Some(node) = graph.node_mut(import_node_id) {
                if let NodeData::Import {
                    resolved_path: ref mut rp,
                    ..
                } = node.data_mut()
                {
                    *rp = Some(resolved_path);
                }
            }
        }
    }

    // ── Module path resolution ───────────────────────────────────────────────

    fn resolve_module(
        &self,
        specifier: &str,
        importing_file: &Path,
        file_index: &HashMap<PathBuf, NodeId>,
    ) -> Option<PathBuf> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            self.resolve_relative(specifier, importing_file, file_index)
        } else {
            self.resolve_absolute(specifier, file_index)
        }
    }

    /// Resolve a relative import path (TypeScript, Python relative imports).
    fn resolve_relative(
        &self,
        specifier: &str,
        importing_file: &Path,
        file_index: &HashMap<PathBuf, NodeId>,
    ) -> Option<PathBuf> {
        let base = importing_file.parent()?;
        let raw = base.join(specifier);
        self.try_with_extensions(&raw, file_index)
    }

    /// Resolve an absolute / non-relative specifier (Python package, Go import path).
    fn resolve_absolute(
        &self,
        specifier: &str,
        file_index: &HashMap<PathBuf, NodeId>,
    ) -> Option<PathBuf> {
        // Python-style: "mypackage.utils" → "mypackage/utils"
        let as_path = specifier.replace('.', "/");
        let candidate = self.root.join(&as_path);
        if let Some(p) = self.try_with_extensions(&candidate, file_index) {
            return Some(p);
        }

        // Go-style: match last path segment as package name
        let last = specifier.split('/').next_back()?;
        file_index
            .keys()
            .find(|p| {
                p.parent()
                    .and_then(|d| d.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| n == last)
                    .unwrap_or(false)
            })
            .cloned()
    }

    /// Try a base path with various source-file extensions; return the first match.
    fn try_with_extensions(
        &self,
        base: &Path,
        file_index: &HashMap<PathBuf, NodeId>,
    ) -> Option<PathBuf> {
        // Exact path (already has extension)
        if file_index.contains_key(base) {
            return Some(base.to_path_buf());
        }

        for ext in &[
            "ts", "tsx", "js", "jsx", "py", "go", "java", "cs", "rs", "rb", "kt", "swift", "php",
        ] {
            let p = base.with_extension(ext);
            if file_index.contains_key(&p) {
                return Some(p);
            }
        }

        // Index-file fallback: dir/index.ts, dir/__init__.py, dir/mod.rs
        for name in &["index.ts", "index.js", "mod.rs", "__init__.py"] {
            let p = base.join(name);
            if file_index.contains_key(&p) {
                return Some(p);
            }
        }

        None
    }
}
