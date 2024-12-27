// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

#[test]
fn test_de_char() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(rmsd_yaml::from_str::<char>("q")?, 'q');
    Ok(())
}

#[test]
fn test_de_str() -> Result<(), Box<dyn std::error::Error>> {
    let yaml_str = r#"
        "acb"
    "#;

    let abc: String = rmsd_yaml::from_str(yaml_str)?;
    assert_eq!(abc, "acb");
    Ok(())
}

#[test]
fn test_de_bool() -> Result<(), Box<dyn std::error::Error>> {
    assert!(!rmsd_yaml::from_str("false")?);

    assert!(rmsd_yaml::from_str("true")?);
    Ok(())
}

#[test]
fn test_de_unsign_number() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(123114u32, rmsd_yaml::from_str("\n---\n123114")?);

    assert_eq!(1234u16, rmsd_yaml::from_str("+1234")?);

    assert_eq!(0x123123u64, rmsd_yaml::from_str("0x123123")?);
    assert_eq!(0o123u16, rmsd_yaml::from_str("0o123")?);
    assert_eq!(0b1001u8, rmsd_yaml::from_str("0b1001")?);

    Ok(())
}

#[test]
fn test_de_struct() -> Result<(), Box<dyn std::error::Error>> {
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

    let yaml_str = r#"
        ---
        uint_a: 500
        str_b: "abc"
        bar:
          data: false
    "#;

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
fn test_de_array() -> Result<(), Box<dyn std::error::Error>> {
    let yaml_str = r#"
        ---
        - 500
        - 600
        - 0xfe
    "#;

    let value: Vec<u32> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(value, vec![500u32, 600, 0xfe,]);
    Ok(())
}

#[test]
fn test_de_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let yaml_str = r#"
        ---
        - 500
        - 0xff
    "#;

    let value: (u32, u32) = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(value, (500u32, 0xff));
    Ok(())
}

#[test]
fn test_de_tuple_of_struct() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
    }

    let yaml_str = r#"
        ---
        - uint_a: 36
          str_b: item1
        - uint_a: 37
          str_b: item2
    "#;

    let value: (FooTest, FooTest) = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        value,
        (
            FooTest {
                uint_a: 36,
                str_b: "item1".to_string(),
            },
            FooTest {
                uint_a: 37,
                str_b: "item2".to_string(),
            },
        )
    );
    Ok(())
}

#[test]
fn test_de_array_of_struct() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
    }

    let yaml_str = r#"
        ---
        - uint_a: 36
          str_b: item1
        - uint_a: 37
          str_b: item2
    "#;

    let value: Vec<FooTest> = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        value,
        vec![
            FooTest {
                uint_a: 36,
                str_b: "item1".to_string(),
            },
            FooTest {
                uint_a: 37,
                str_b: "item2".to_string(),
            },
        ]
    );
    Ok(())
}

#[test]
fn test_de_new_type_struct() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest(String);

    let yaml_str = r#"
        ---
        abcedfo
    "#;

    let value: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(value, FooTest("abcedfo".to_string()));
    Ok(())
}
