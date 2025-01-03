// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

use rmsd_yaml::RmsdError;

#[test]
fn test_de_yaml_flow_style_struct() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bar: BarTest,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        data: bool,
    }

    let yaml_str = r#"{ uint_a: 500, str_b: "abc", bar: {data: false}}"#;

    let foo_test: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        FooTest {
            uint_a: 500,
            str_b: "abc".to_string(),
            bar: BarTest { data: false }
        }
    );
    Ok(())
}

#[test]
fn test_de_yaml_mix_flow_and_block_style_on_dict() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bar: BarTest,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        data: bool,
    }

    let yaml_str = r#"{ uint_a: 500, str_b: "abc",
    bar: {data: false}}"#;

    let foo_test: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        FooTest {
            uint_a: 500,
            str_b: "abc".to_string(),
            bar: BarTest { data: false }
        }
    );
    Ok(())
}

#[test]
fn test_de_json_struct() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bar: BarTest,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        data: bool,
    }

    let yaml_str = r#"{
  "uint_a": 500,
  "str_b": "abc",
  "bar": {
    "data": false
  }
}"#;

    let foo_test: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        FooTest {
            uint_a: 500,
            str_b: "abc".to_string(),
            bar: BarTest { data: false }
        }
    );
    Ok(())
}

#[test]
fn test_de_yaml_flow_style_array() -> Result<(), RmsdError> {
    let yaml_str = r#"[1,2,3,4]"#;

    let foo_test: Vec<u8> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(foo_test, vec![1u8, 2, 3, 4]);
    Ok(())
}

#[test]
fn test_de_yaml_flow_style_nested_array() -> Result<(), RmsdError> {
    let yaml_str = r#"[[1,2,3,4], [2,3,4,5]]"#;

    let foo_test: Vec<Vec<u8>> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(foo_test, vec![vec![1u8, 2, 3, 4], vec![2u8, 3, 4, 5]]);
    Ok(())
}

#[test]
fn test_de_yaml_mix_flow_and_block_array() -> Result<(), RmsdError> {
    let yaml_str = r#"
    - [1,2,3,4]
    - [2,3,4,5]"#;

    let foo_test: Vec<Vec<u8>> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(foo_test, vec![vec![1u8, 2, 3, 4], vec![2u8, 3, 4, 5]]);
    Ok(())
}

#[test]
fn test_de_yaml_flow_array_of_struct() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
    }
    let yaml_str = r#"
    [{uint_a: 3, str_b: iterm1 },
     {uint_a: 4, str_b: iterm2 }]
    "#;

    let foo_test: Vec<FooTest> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        vec![
            FooTest {
                uint_a: 3,
                str_b: "iterm1".to_string(),
            },
            FooTest {
                uint_a: 4,
                str_b: "iterm2".to_string(),
            }
        ]
    );
    Ok(())
}

#[test]
fn test_de_yaml_flow_struct_of_array() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        #[serde(rename = "uint-a")]
        uint_a: Vec<u32>,
    }
    let yaml_str = r#"{uint-a: [1, 2, 3, 4]}"#;

    let foo_test: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        FooTest {
            uint_a: vec![1, 2, 3, 4]
        }
    );
    Ok(())
}
