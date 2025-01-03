// SPDX-License-Identifier: Apache-2.0

use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct FooTest {
    uint_a: u32,
    str_b: String,
    bar: BarTest,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct BarTest {
    data: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = vec![
        FooTest {
            uint_a: 128,
            str_b: "foo".into(),
            bar: BarTest { data: false },
        },
        FooTest {
            uint_a: 129,
            str_b: "bar".into(),
            bar: BarTest { data: true },
        },
    ];
    println!("{}", rmsd_yaml::to_string(&data)?);
    Ok(())
}
