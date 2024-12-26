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
    assert_eq!(123114u32, rmsd_yaml::from_str("123114")?);

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
