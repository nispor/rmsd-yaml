// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct FooTest {
    uint_a: u32,
    str_b: String,
}

#[test]
fn test_from_str_to_struct() -> Result<(), Box<dyn std::error::Error>> {
    let yaml_str = r#"
        unit_a: 500
        str_b: "abc"
    "#;

    let foo_test: FooTest = rmsd_yaml::from_str(yaml_str)?;

    assert_eq!(
        foo_test,
        FooTest {
            uint_a: 500,
            str_b: "abc".to_string()
        }
    );
    Ok(())
}
