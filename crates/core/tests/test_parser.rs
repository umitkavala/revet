//! Tests for parser dispatcher

use revet_core::ParserDispatcher;
use std::path::PathBuf;

#[test]
fn test_dispatcher_creation() {
    let dispatcher = ParserDispatcher::new();
    let extensions = dispatcher.supported_extensions();
    assert!(!extensions.is_empty());
}

#[test]
fn test_find_python_parser() {
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py"));
    assert!(parser.is_some());
    assert_eq!(parser.unwrap().language_name(), "python");
}
