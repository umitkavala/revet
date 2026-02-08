//! CozoDB-backed graph store
//!
//! Provides persistent, indexed graph storage using CozoDB with SQLite backend.
//! Behind the `cozo-store` feature flag.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use cozo_ce::{DataValue, DbInstance, NamedRows, Num, ScriptMutability};

use crate::graph::{Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeKind};
use crate::CodeGraph;

use super::{EdgeResult, GraphStore, SnapshotInfo, StoreNodeId};

/// CozoDB-backed graph store
pub struct CozoStore {
    db: DbInstance,
}

impl CozoStore {
    /// Create a new in-memory CozoStore (for tests)
    pub fn new_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", Default::default())
            .map_err(|e| anyhow::anyhow!("failed to create CozoDB: {e}"))?;
        let store = Self { db };
        store.init_schema()?;
        Ok(store)
    }

    /// Create a new SQLite-backed CozoStore (for persistence)
    pub fn new_sqlite(path: impl AsRef<Path>) -> Result<Self> {
        let db = DbInstance::new("sqlite", path.as_ref(), Default::default())
            .map_err(|e| anyhow::anyhow!("failed to create CozoDB with SQLite: {e}"))?;
        let store = Self { db };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the stored relations (tables)
    fn init_schema(&self) -> Result<()> {
        // Create nodes relation
        let create_nodes = r#"
            :create nodes {
                snapshot: String,
                id: Int
                =>
                kind: String,
                name: String,
                file_path: String,
                line: Int,
                end_line: Int,
                data_json: String
            }
        "#;

        // Create edges relation
        let create_edges = r#"
            :create edges {
                snapshot: String,
                from_id: Int,
                to_id: Int,
                edge_idx: Int
                =>
                kind: String,
                metadata_json: String
            }
        "#;

        // Create snapshots metadata relation
        let create_snapshots = r#"
            :create snapshots {
                name: String
                =>
                node_count: Int,
                edge_count: Int
            }
        "#;

        for script in [create_nodes, create_edges, create_snapshots] {
            match self.run_mut(script) {
                Ok(_) => {}
                Err(e) => {
                    let msg = format!("{e}");
                    // Ignore "already exists" errors on schema creation
                    if !msg.contains("already exists") {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Run a mutable CozoScript
    fn run_mut(&self, script: &str) -> Result<NamedRows> {
        self.db
            .run_script(script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Run an immutable CozoScript with parameters
    fn run_query(&self, script: &str, params: BTreeMap<String, DataValue>) -> Result<NamedRows> {
        self.db
            .run_script(script, params, ScriptMutability::Immutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Run a mutable CozoScript with parameters
    fn run_mut_with(&self, script: &str, params: BTreeMap<String, DataValue>) -> Result<NamedRows> {
        self.db
            .run_script(script, params, ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Deserialize a Node from a CozoDB row
    /// Expected columns: id, kind, name, file_path, line, end_line, data_json
    fn deserialize_node(row: &[DataValue]) -> Result<(StoreNodeId, Node)> {
        let id = row_int(&row[0])? as u64;
        let kind_str = row_str(&row[1])?;
        let name = row_str(&row[2])?.to_string();
        let file_path = row_str(&row[3])?.into();
        let line = row_int(&row[4])? as usize;
        let end_line_val = row_int(&row[5])?;
        let data_json = row_str(&row[6])?;

        let kind: NodeKind =
            serde_json::from_str(&format!("\"{kind_str}\"")).context("invalid NodeKind")?;
        let data: NodeData = serde_json::from_str(data_json).context("invalid NodeData")?;

        let mut node = Node::new(kind, name, file_path, line, data);
        if end_line_val >= 0 {
            node.set_end_line(end_line_val as usize);
        }

        Ok((StoreNodeId(id), node))
    }

    /// Deserialize an Edge from a CozoDB row
    /// Expected columns: from_id, to_id, kind, metadata_json
    fn deserialize_edge(row: &[DataValue]) -> Result<EdgeResult> {
        let from_id = row_int(&row[0])? as u64;
        let to_id = row_int(&row[1])? as u64;
        let kind_str = row_str(&row[2])?;
        let metadata_json = row_str(&row[3])?;

        let kind: EdgeKind =
            serde_json::from_str(&format!("\"{kind_str}\"")).context("invalid EdgeKind")?;
        let metadata: Option<EdgeMetadata> = if metadata_json.is_empty() || metadata_json == "null"
        {
            None
        } else {
            Some(serde_json::from_str(metadata_json).context("invalid EdgeMetadata")?)
        };

        let edge = match metadata {
            Some(m) => Edge::with_metadata(kind, m),
            None => Edge::new(kind),
        };

        Ok(EdgeResult {
            from: StoreNodeId(from_id),
            to: StoreNodeId(to_id),
            edge,
        })
    }
}

/// Extract i64 from DataValue
fn row_int(val: &DataValue) -> Result<i64> {
    match val {
        DataValue::Num(Num::Int(i)) => Ok(*i),
        DataValue::Num(Num::Float(f)) => Ok(*f as i64),
        _ => anyhow::bail!("expected integer, got {val:?}"),
    }
}

/// Extract &str from DataValue
fn row_str(val: &DataValue) -> Result<&str> {
    match val {
        DataValue::Str(s) => Ok(s.as_str()),
        _ => anyhow::bail!("expected string, got {val:?}"),
    }
}

/// Serialize an enum variant to its string name via serde
fn kind_to_string<T: serde::Serialize>(kind: &T) -> String {
    // serde_json serializes unit enum variants as quoted strings like "\"Function\""
    let json = serde_json::to_string(kind).unwrap_or_default();
    // Strip surrounding quotes
    json.trim_matches('"').to_string()
}

impl GraphStore for CozoStore {
    fn flush(&self, graph: &CodeGraph, snapshot: &str) -> Result<()> {
        // Delete existing snapshot data first
        self.delete_snapshot(snapshot)?;

        // Build node rows as DataValue vectors for import_relations
        let mut node_rows: Vec<Vec<DataValue>> = Vec::new();
        for (node_id, node) in graph.nodes() {
            let data_json = serde_json::to_string(node.data())?;
            let end_line = node.end_line().map(|l| l as i64).unwrap_or(-1);

            node_rows.push(vec![
                DataValue::Str(snapshot.into()),
                DataValue::from(node_id.index() as i64),
                DataValue::Str(kind_to_string(node.kind()).into()),
                DataValue::Str(node.name().into()),
                DataValue::Str(node.file_path().display().to_string().into()),
                DataValue::from(node.line() as i64),
                DataValue::from(end_line),
                DataValue::Str(data_json.into()),
            ]);
        }

        // Build edge rows
        let mut edge_rows: Vec<Vec<DataValue>> = Vec::new();
        let mut edge_count: i64 = 0;
        for (node_id, _) in graph.nodes() {
            let from_id = node_id.index() as i64;
            for (edge_idx_counter, (target, edge)) in (0_i64..).zip(graph.edges_from(node_id)) {
                let to_id = target.index() as i64;
                let metadata_json = match edge.metadata() {
                    Some(m) => serde_json::to_string(m)?,
                    None => String::new(),
                };

                edge_rows.push(vec![
                    DataValue::Str(snapshot.into()),
                    DataValue::from(from_id),
                    DataValue::from(to_id),
                    DataValue::from(edge_idx_counter),
                    DataValue::Str(kind_to_string(edge.kind()).into()),
                    DataValue::Str(metadata_json.into()),
                ]);

                edge_count += 1;
            }
        }

        // Use import_relations for bulk insertion (avoids CozoScript string escaping)
        let node_count = node_rows.len() as i64;

        if !node_rows.is_empty() {
            let mut data = BTreeMap::new();
            data.insert(
                "nodes".to_string(),
                NamedRows {
                    headers: vec![
                        "snapshot".into(),
                        "id".into(),
                        "kind".into(),
                        "name".into(),
                        "file_path".into(),
                        "line".into(),
                        "end_line".into(),
                        "data_json".into(),
                    ],
                    rows: node_rows,
                    next: None,
                },
            );
            self.db
                .import_relations(data)
                .map_err(|e| anyhow::anyhow!("failed to import nodes: {e}"))?;
        }

        if !edge_rows.is_empty() {
            let mut data = BTreeMap::new();
            data.insert(
                "edges".to_string(),
                NamedRows {
                    headers: vec![
                        "snapshot".into(),
                        "from_id".into(),
                        "to_id".into(),
                        "edge_idx".into(),
                        "kind".into(),
                        "metadata_json".into(),
                    ],
                    rows: edge_rows,
                    next: None,
                },
            );
            self.db
                .import_relations(data)
                .map_err(|e| anyhow::anyhow!("failed to import edges: {e}"))?;
        }

        // Record snapshot metadata
        let mut snap_data = BTreeMap::new();
        snap_data.insert(
            "snapshots".to_string(),
            NamedRows {
                headers: vec!["name".into(), "node_count".into(), "edge_count".into()],
                rows: vec![vec![
                    DataValue::Str(snapshot.into()),
                    DataValue::from(node_count),
                    DataValue::from(edge_count),
                ]],
                next: None,
            },
        );
        self.db
            .import_relations(snap_data)
            .map_err(|e| anyhow::anyhow!("failed to import snapshot metadata: {e}"))?;

        Ok(())
    }

    fn snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let result = self.run_query(
            "?[name, node_count, edge_count] := *snapshots{name, node_count, edge_count}",
            BTreeMap::new(),
        )?;

        result
            .rows
            .iter()
            .map(|row| {
                Ok(SnapshotInfo {
                    name: row_str(&row[0])?.to_string(),
                    node_count: row_int(&row[1])? as usize,
                    edge_count: row_int(&row[2])? as usize,
                })
            })
            .collect()
    }

    fn delete_snapshot(&self, snapshot: &str) -> Result<()> {
        let snap_val = DataValue::Str(snapshot.into());

        // Delete nodes
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), snap_val.clone());
        let _ = self.run_mut_with(
            r#"?[snapshot, id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap
            :delete nodes {snapshot, id => kind, name, file_path, line, end_line, data_json}"#,
            params,
        );

        // Delete edges
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), snap_val.clone());
        let _ = self.run_mut_with(
            r#"?[snapshot, from_id, to_id, edge_idx, kind, metadata_json] :=
                *edges{snapshot, from_id, to_id, edge_idx, kind, metadata_json},
                snapshot = $snap
            :delete edges {snapshot, from_id, to_id, edge_idx => kind, metadata_json}"#,
            params,
        );

        // Delete snapshot metadata
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), snap_val);
        let _ = self.run_mut_with(
            r#"?[name, node_count, edge_count] :=
                *snapshots{name, node_count, edge_count},
                name = $snap
            :delete snapshots {name => node_count, edge_count}"#,
            params,
        );

        Ok(())
    }

    fn node(&self, id: StoreNodeId, snapshot: &str) -> Result<Option<Node>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("id".to_string(), DataValue::from(id.0 as i64));

        let result = self.run_query(
            r#"?[id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap, id = $id"#,
            params,
        )?;

        if result.rows.is_empty() {
            return Ok(None);
        }

        let (_, node) = Self::deserialize_node(&result.rows[0])?;
        Ok(Some(node))
    }

    fn nodes(&self, snapshot: &str) -> Result<Vec<(StoreNodeId, Node)>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));

        let result = self.run_query(
            r#"?[id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Self::deserialize_node(row))
            .collect()
    }

    fn find_nodes(
        &self,
        file_path: &str,
        name: Option<&str>,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("fp".to_string(), DataValue::Str(file_path.into()));

        let script = if let Some(n) = name {
            params.insert("name".to_string(), DataValue::Str(n.into()));
            r#"?[id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap, file_path = $fp, name = $name"#
        } else {
            r#"?[id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap, file_path = $fp"#
        };

        let result = self.run_query(script, params)?;
        result
            .rows
            .iter()
            .map(|row| Self::deserialize_node(row))
            .collect()
    }

    fn find_nodes_by_kind(
        &self,
        kind: NodeKind,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert(
            "kind".to_string(),
            DataValue::Str(kind_to_string(&kind).into()),
        );

        let result = self.run_query(
            r#"?[id, kind, name, file_path, line, end_line, data_json] :=
                *nodes{snapshot, id, kind, name, file_path, line, end_line, data_json},
                snapshot = $snap, kind = $kind"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Self::deserialize_node(row))
            .collect()
    }

    fn node_count(&self, snapshot: &str) -> Result<usize> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));

        let result = self.run_query(
            r#"?[count(id)] := *nodes{snapshot, id}, snapshot = $snap"#,
            params,
        )?;

        if result.rows.is_empty() {
            return Ok(0);
        }
        Ok(row_int(&result.rows[0][0])? as usize)
    }

    fn edges_from(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("from".to_string(), DataValue::from(node.0 as i64));

        let result = self.run_query(
            r#"?[from_id, to_id, kind, metadata_json] :=
                *edges{snapshot, from_id, to_id, kind, metadata_json},
                snapshot = $snap, from_id = $from"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Self::deserialize_edge(row))
            .collect()
    }

    fn edges_to(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("to".to_string(), DataValue::from(node.0 as i64));

        let result = self.run_query(
            r#"?[from_id, to_id, kind, metadata_json] :=
                *edges{snapshot, from_id, to_id, kind, metadata_json},
                snapshot = $snap, to_id = $to"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Self::deserialize_edge(row))
            .collect()
    }

    fn direct_dependents(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("target".to_string(), DataValue::from(node.0 as i64));

        let result = self.run_query(
            r#"?[from_id] := *edges{snapshot, from_id, to_id},
                snapshot = $snap, to_id = $target"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Ok(StoreNodeId(row_int(&row[0])? as u64)))
            .collect()
    }

    fn transitive_dependents(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        // Use recursive Datalog for transitive closure
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("start".to_string(), DataValue::from(node.0 as i64));

        let script = if let Some(depth) = max_depth {
            // With depth limit: track depth in the recursion
            params.insert("max_depth".to_string(), DataValue::from(depth as i64));
            r#"dep[x, d] := *edges{snapshot, from_id: x, to_id: $start}, snapshot = $snap, d = 1
               dep[x, d] := dep[y, d1], *edges{snapshot, from_id: x, to_id: y}, snapshot = $snap, d = d1 + 1, d <= $max_depth
               ?[x] := dep[x, _]"#
        } else {
            r#"dep[x] := *edges{snapshot, from_id: x, to_id: $start}, snapshot = $snap
               dep[x] := dep[y], *edges{snapshot, from_id: x, to_id: y}, snapshot = $snap
               ?[x] := dep[x]"#
        };

        let result = self.run_query(script, params)?;

        result
            .rows
            .iter()
            .map(|row| Ok(StoreNodeId(row_int(&row[0])? as u64)))
            .collect()
    }

    fn dependencies(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("source".to_string(), DataValue::from(node.0 as i64));

        let result = self.run_query(
            r#"?[to_id] := *edges{snapshot, from_id, to_id},
                snapshot = $snap, from_id = $source"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Ok(StoreNodeId(row_int(&row[0])? as u64)))
            .collect()
    }

    fn transitive_dependencies(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("start".to_string(), DataValue::from(node.0 as i64));

        let script = if let Some(depth) = max_depth {
            params.insert("max_depth".to_string(), DataValue::from(depth as i64));
            r#"dep[x, d] := *edges{snapshot, from_id: $start, to_id: x}, snapshot = $snap, d = 1
               dep[x, d] := dep[y, d1], *edges{snapshot, from_id: y, to_id: x}, snapshot = $snap, d = d1 + 1, d <= $max_depth
               ?[x] := dep[x, _]"#
        } else {
            r#"dep[x] := *edges{snapshot, from_id: $start, to_id: x}, snapshot = $snap
               dep[x] := dep[y], *edges{snapshot, from_id: y, to_id: x}, snapshot = $snap
               ?[x] := dep[x]"#
        };

        let result = self.run_query(script, params)?;

        result
            .rows
            .iter()
            .map(|row| Ok(StoreNodeId(row_int(&row[0])? as u64)))
            .collect()
    }

    fn find_by_edge_kind(
        &self,
        node: StoreNodeId,
        kind: EdgeKind,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        let mut params = BTreeMap::new();
        params.insert("snap".to_string(), DataValue::Str(snapshot.into()));
        params.insert("source".to_string(), DataValue::from(node.0 as i64));
        params.insert(
            "kind".to_string(),
            DataValue::Str(kind_to_string(&kind).into()),
        );

        let result = self.run_query(
            r#"?[to_id] := *edges{snapshot, from_id, to_id, kind},
                snapshot = $snap, from_id = $source, kind = $kind"#,
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| Ok(StoreNodeId(row_int(&row[0])? as u64)))
            .collect()
    }

    fn find_changed_nodes(
        &self,
        old_snapshot: &str,
        new_snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Option<StoreNodeId>)>> {
        let mut params = BTreeMap::new();
        params.insert("new_snap".to_string(), DataValue::Str(new_snapshot.into()));
        params.insert("old_snap".to_string(), DataValue::Str(old_snapshot.into()));

        // Find modified nodes: same file_path + name + kind but different data or line
        // Find added nodes: exist in new but not in old
        // Split into separate queries for line changes and data changes to avoid 'or'
        let script = r#"
            changed[new_id, old_id] :=
                *nodes{snapshot: $new_snap, id: new_id, kind, name, file_path, line: new_line, data_json},
                *nodes{snapshot: $old_snap, id: old_id, kind, name, file_path, line: old_line, data_json},
                new_line != old_line
            changed[new_id, old_id] :=
                *nodes{snapshot: $new_snap, id: new_id, kind, name, file_path, data_json: new_data},
                *nodes{snapshot: $old_snap, id: old_id, kind, name, file_path, data_json: old_data},
                new_data != old_data
            added[new_id] :=
                *nodes{snapshot: $new_snap, id: new_id, kind, name, file_path},
                not *nodes{snapshot: $old_snap, kind, name, file_path}
            ?[new_id, old_id] := changed[new_id, old_id]
            ?[new_id, old_id] := added[new_id], old_id = -1
        "#;

        let result = self.run_query(script, params)?;

        result
            .rows
            .iter()
            .map(|row| {
                let new_id = StoreNodeId(row_int(&row[0])? as u64);
                let old_id_val = row_int(&row[1])?;
                let old_id = if old_id_val < 0 {
                    None
                } else {
                    Some(StoreNodeId(old_id_val as u64))
                };
                Ok((new_id, old_id))
            })
            .collect()
    }
}
