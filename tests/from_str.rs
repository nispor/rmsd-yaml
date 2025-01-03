// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;
use serde::{Deserialize, Serialize};

use rmsd_yaml::RmsdError;

#[test]
fn test_de_char() -> Result<(), RmsdError> {
    assert_eq!(rmsd_yaml::from_str::<char>("q")?, 'q');
    Ok(())
}

#[test]
fn test_de_str() -> Result<(), RmsdError> {
    let yaml_str = r#"
        "acb"
    "#;

    let abc: String = rmsd_yaml::from_str(yaml_str)?;
    assert_eq!(abc, "acb");
    Ok(())
}

#[test]
fn test_de_bool() -> Result<(), RmsdError> {
    assert!(!rmsd_yaml::from_str("false")?);

    assert!(rmsd_yaml::from_str("true")?);
    Ok(())
}

#[test]
fn test_de_unsign_number() -> Result<(), RmsdError> {
    assert_eq!(123114u32, rmsd_yaml::from_str("\n---\n123114")?);

    assert_eq!(1234u16, rmsd_yaml::from_str("+1234")?);

    assert_eq!(0x123123u64, rmsd_yaml::from_str("0x123123")?);
    assert_eq!(0o123u16, rmsd_yaml::from_str("0o123")?);
    assert_eq!(0b1001u8, rmsd_yaml::from_str("0b1001")?);

    Ok(())
}

#[test]
fn test_de_struct() -> Result<(), RmsdError> {
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
fn test_de_array() -> Result<(), RmsdError> {
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
fn test_de_tuple() -> Result<(), RmsdError> {
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
fn test_de_tuple_of_struct() -> Result<(), RmsdError> {
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
fn test_de_array_of_struct() -> Result<(), RmsdError> {
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
fn test_de_new_type_struct() -> Result<(), RmsdError> {
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

#[test]
fn test_de_enum() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    enum FooTest {
        Abc,
        Abd,
        Abe,
    }

    assert_eq!(FooTest::Abe, rmsd_yaml::from_str("Abe")?);

    Ok(())
}

#[test]
fn test_de_enum_new_type_struct() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    enum FooTest {
        Abc(u32),
        Abd(u64),
        Abe(u8),
    }

    assert_eq!(
        FooTest::Abe(128),
        rmsd_yaml::from_str::<FooTest>("!Abe 128")?
    );

    Ok(())
}

#[test]
fn test_de_enum_new_type_struct_string() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    enum FooTest {
        Abc(String),
        Abd(String),
        Abe(String),
    }

    assert_eq!(
        FooTest::Abe("128".into()),
        rmsd_yaml::from_str::<FooTest>("!Abe 128")?
    );

    Ok(())
}

#[test]
fn test_de_array_of_enum_of_struct() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: u32,
        str_b: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        uint_b: u32,
        str_c: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    enum EnumTest {
        Foo(FooTest),
        Bar(BarTest),
    }

    assert_eq!(
        vec![
            EnumTest::Foo(FooTest {
                uint_a: 128,
                str_b: "foo".into(),
            }),
            EnumTest::Bar(BarTest {
                uint_b: 129,
                str_c: "bar".into(),
            }),
        ],
        rmsd_yaml::from_str::<Vec<EnumTest>>(
            r#"
            ---
            - !Foo
              uint_a: 128
              str_b: foo
            - !Bar
              uint_b: 129
              str_c: bar
            "#
        )?
    );

    Ok(())
}

#[test]
fn test_de_struct_with_enum_member() -> Result<(), RmsdError> {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct FooTest {
        uint_a: Option<EnumTest>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    enum EnumTest {
        Bar(BarTest),
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    struct BarTest {
        uint_b: u32,
    }

    assert_eq!(
        FooTest {
            uint_a: Some(EnumTest::Bar(BarTest { uint_b: 32 }))
        },
        rmsd_yaml::from_str::<FooTest>(
            r#"
            ---
            uint_a:
              !Bar
              uint_b: 32
            "#
        )?
    );

    Ok(())
}
