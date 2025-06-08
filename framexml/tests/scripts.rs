#![feature(assert_matches)]

use framexml::scripts::{ScriptContent, convert_script_item};
use framexml_parser::scripts::ScriptItemType;
use std::assert_matches::assert_matches;

#[test]
/// This test is a bit stupid as it is literally just testing the implementation of the converter.
fn script_converter() {
    let item_none = ScriptItemType {
        content: None,
        function: None,
    };

    let none = convert_script_item(item_none);
    assert!(none.is_err());

    let item_both = ScriptItemType {
        content: Some("Test Content".to_string()),
        function: Some("Test".to_string()),
    };

    let both_result = convert_script_item(item_both).unwrap();
    assert!(matches!(both_result, ScriptContent::Text(_)));
    if let ScriptContent::Text(content) = both_result {
        assert_eq!(content, "Test Content");
    }

    let item_content = ScriptItemType {
        content: Some("Test Content".to_string()),
        function: None,
    };
    let content_result = convert_script_item(item_content).unwrap();
    assert_matches!(content_result, ScriptContent::Text(_));
    if let ScriptContent::Text(content) = content_result {
        assert_eq!(content, "Test Content");
    }

    let item_function = ScriptItemType {
        content: None,
        function: Some("Test".to_string()),
    };
    let function_result = convert_script_item(item_function).unwrap();
    assert_matches!(function_result, ScriptContent::FunctionReference(_));
    if let ScriptContent::FunctionReference(function) = function_result {
        assert_eq!(function, "Test");
    }
}
