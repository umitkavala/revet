//! Tests for impact analysis

use revet_core::graph::{CodeGraph, Edge, EdgeKind, Node, NodeData, NodeKind, Parameter};
use revet_core::{ChangeClassification, ImpactAnalysis};
use std::path::PathBuf;

#[test]
fn test_impact_analysis_basic() {
    // Create old graph
    let mut old_graph = CodeGraph::new(PathBuf::from("/test"));

    let old_func_a = old_graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let old_func_b = old_graph.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    // B depends on A
    old_graph.add_edge(old_func_b, old_func_a, Edge::new(EdgeKind::Calls));

    // Create new graph with modified func_a
    let mut new_graph = CodeGraph::new(PathBuf::from("/test"));

    let new_func_a = new_graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![Parameter {
                name: "x".to_string(),
                param_type: Some("int".to_string()),
                default_value: None,
            }],
            return_type: Some("int".to_string()),
        },
    ));

    let new_func_b = new_graph.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    new_graph.add_edge(new_func_b, new_func_a, Edge::new(EdgeKind::Calls));

    let analyzer = ImpactAnalysis::new(old_graph, new_graph);
    let report = analyzer.analyze_impact();

    // Should detect func_a as changed
    assert!(report.changes.len() >= 1);

    // Should classify as breaking (signature changed)
    let breaking_changes: Vec<_> = report.breaking_changes().collect();
    assert!(breaking_changes.len() >= 1);

    // Should find func_b as a dependent
    let func_a_impact = &report.changes.iter().find(|c| {
        analyzer
            .new_graph()
            .node(c.node_id)
            .map(|n| n.name() == "func_a")
            .unwrap_or(false)
    });

    if let Some(impact) = func_a_impact {
        assert!(impact.direct_dependents.len() >= 1);
    }
}

#[test]
fn test_compare_function_signatures() {
    let old_graph = CodeGraph::new(PathBuf::from("/test"));
    let new_graph = CodeGraph::new(PathBuf::from("/test"));
    let analyzer = ImpactAnalysis::new(old_graph, new_graph);

    // Test 1: Adding parameter without default = BREAKING
    let old_params = vec![];
    let new_params = vec![Parameter {
        name: "x".to_string(),
        param_type: Some("int".to_string()),
        default_value: None,
    }];

    let result = analyzer.compare_function_signatures(&old_params, &None, &new_params, &None);
    assert_eq!(result, ChangeClassification::Breaking);

    // Test 2: Adding parameter with default = POTENTIALLY BREAKING
    let new_params_with_default = vec![Parameter {
        name: "x".to_string(),
        param_type: Some("int".to_string()),
        default_value: Some("0".to_string()),
    }];

    let result = analyzer.compare_function_signatures(
        &old_params,
        &None,
        &new_params_with_default,
        &None,
    );
    assert_eq!(result, ChangeClassification::PotentiallyBreaking);

    // Test 3: Return type changed = BREAKING
    let result = analyzer.compare_function_signatures(
        &old_params,
        &None,
        &old_params,
        &Some("int".to_string()),
    );
    assert_eq!(result, ChangeClassification::Breaking);

    // Test 4: No changes = SAFE
    let result = analyzer.compare_function_signatures(&old_params, &None, &old_params, &None);
    assert_eq!(result, ChangeClassification::Safe);
}

#[test]
fn test_compare_classes() {
    let old_graph = CodeGraph::new(PathBuf::from("/test"));
    let new_graph = CodeGraph::new(PathBuf::from("/test"));
    let analyzer = ImpactAnalysis::new(old_graph, new_graph);

    let old_bases = vec!["Base".to_string()];
    let old_methods = vec!["method_a".to_string(), "method_b".to_string()];
    let old_fields = vec!["field_x".to_string()];

    // Test 1: Removed method = BREAKING
    let new_methods_removed = vec!["method_a".to_string()];
    let result = analyzer.compare_classes(
        &old_bases,
        &old_methods,
        &old_fields,
        &old_bases,
        &new_methods_removed,
        &old_fields,
    );
    assert_eq!(result, ChangeClassification::Breaking);

    // Test 2: Added method = SAFE
    let new_methods_added = vec![
        "method_a".to_string(),
        "method_b".to_string(),
        "method_c".to_string(),
    ];
    let result = analyzer.compare_classes(
        &old_bases,
        &old_methods,
        &old_fields,
        &old_bases,
        &new_methods_added,
        &old_fields,
    );
    assert_eq!(result, ChangeClassification::Safe);

    // Test 3: Changed base class = BREAKING
    let new_bases = vec!["NewBase".to_string()];
    let result = analyzer.compare_classes(
        &old_bases,
        &old_methods,
        &old_fields,
        &new_bases,
        &old_methods,
        &old_fields,
    );
    assert_eq!(result, ChangeClassification::Breaking);

    // Test 4: Removed field = BREAKING
    let new_fields_removed = vec![];
    let result = analyzer.compare_classes(
        &old_bases,
        &old_methods,
        &old_fields,
        &old_bases,
        &old_methods,
        &new_fields_removed,
    );
    assert_eq!(result, ChangeClassification::Breaking);
}
