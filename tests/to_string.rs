// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;
use rmsd_yaml::{RmsdError, RmsdSerializeOption};
use serde::{Deserialize, Serialize};

#[test]
fn test_struct_to_string() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bool_c: bool,
    }

    let data = FooTest {
        uint_a: 129,
        str_b: "abc".into(),
        bool_c: false,
    };

    let yaml_str = rmsd_yaml::to_string(&data, Default::default())?;

    assert_eq!(
        yaml_str,
        r#"uint_a: 129
str_b: abc
bool_c: false"#
    );

    assert_eq!(rmsd_yaml::from_str::<FooTest>(&yaml_str)?, data);
    Ok(())
}

#[test]
fn test_array_to_string() -> Result<(), RmsdError> {
    let data = vec![1u8, 2, 3, 4];
    let yaml_str = rmsd_yaml::to_string(&data, Default::default())?;

    assert_eq!(yaml_str, "- 1\n- 2\n- 3\n- 4");

    assert_eq!(rmsd_yaml::from_str::<Vec<u8>>(&yaml_str)?, data);
    Ok(())
}

#[test]
fn test_array_of_map() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bool_c: bool,
    }

    let data = [
        FooTest {
            uint_a: 129,
            str_b: "abc".into(),
            bool_c: false,
        },
        FooTest {
            uint_a: 128,
            str_b: "abd".into(),
            bool_c: true,
        },
    ];
    let mut opt = RmsdSerializeOption::default();
    opt.leading_start_indicator = true;

    let yaml_str = rmsd_yaml::to_string(&data, opt)?;

    assert_eq!(
        yaml_str,
        r#"---
- uint_a: 129
  str_b: abc
  bool_c: false
- uint_a: 128
  str_b: abd
  bool_c: true"#
    );

    assert_eq!(rmsd_yaml::from_str::<Vec<FooTest>>(&yaml_str)?, data);
    Ok(())
}

#[test]
fn test_array_in_map() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: Vec<u32>,
        str_b: String,
    }

    let data = FooTest {
        uint_a: vec![129, 128, 127],
        str_b: "abc".into(),
    };
    let mut opt = RmsdSerializeOption::default();
    opt.leading_start_indicator = true;

    let yaml_str = rmsd_yaml::to_string(&data, opt)?;

    assert_eq!(
        yaml_str,
        r#"---
uint_a:
  - 129
  - 128
  - 127
str_b: abc"#
    );

    assert_eq!(rmsd_yaml::from_str::<FooTest>(&yaml_str)?, data);
    Ok(())
}

#[test]
fn test_nested_array() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest(Vec<Vec<u32>>);
    let data = FooTest(vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
    let yaml_str = rmsd_yaml::to_string(&data, Default::default())?;

    assert_eq!(
        yaml_str,
        r#"!FooTest
- - 1
  - 2
  - 3
  - 4
- - 5
  - 6
  - 7
  - 8"#
    );

    assert_eq!(rmsd_yaml::from_str::<FooTest>(&yaml_str)?, data);
    Ok(())
}

#[test]
fn test_nested_map() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
        bar: BarTest,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        data: Vec<u32>,
    }
    let data = FooTest {
        uint_a: 129,
        str_b: "abc".into(),
        bar: BarTest {
            data: vec![1, 2, 3, 4],
        },
    };

    let yaml_str = rmsd_yaml::to_string(&data, Default::default())?;

    assert_eq!(
        yaml_str,
        r#"uint_a: 129
str_b: abc
bar:
  data:
    - 1
    - 2
    - 3
    - 4"#
    );

    assert_eq!(rmsd_yaml::from_str::<FooTest>(&yaml_str)?, data);
    Ok(())
}
