// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use rmsd_yaml::YamlTreeParser;

const TEST_DATA_FOLDER_PATH: &str = "tests/yaml-test-suit-data";
const DESCRIPTION_FILE_NAME: &str = "===";
const INPUT_YAML_FILE_NAME: &str = "in.yaml";

#[test]
fn yaml_test_suit() {
    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(TEST_DATA_FOLDER_PATH);

    for entry in std::fs::read_dir(test_data_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if path.join(DESCRIPTION_FILE_NAME).exists() {
                run_test(&path);
            } else {
                for dir_entry in std::fs::read_dir(&path).unwrap() {
                    let entry = dir_entry.unwrap();
                    let path = entry.path();
                    if path.join(DESCRIPTION_FILE_NAME).exists() {
                        run_test(&path);
                    }
                }
            }
        }
    }
}

fn run_test(path: &Path) {
    let test_name = read_file(&path.join(DESCRIPTION_FILE_NAME));
    let input_yaml = read_file(&path.join(INPUT_YAML_FILE_NAME));
    println!("HAHA518 {:?}", test_name);
}

fn read_file(path: &Path) -> String {
    println!("HAHA639 {:?}", path);
    std::fs::read_to_string(path).unwrap()
}
