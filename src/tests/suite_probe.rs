// Temporary probe: run every supported yaml-test-suite case and report
// pass/fail without stopping at the first failure.
use std::path::Path;

use crate::YamlParser;

use super::yaml_test_suite::SUPPORTED_TESTS;

#[test]
fn suite_probe() {
    super::testlib::init_logger();
    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("yaml-test-suit-data/name");

    let mut test_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&test_data_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if path.join("===").exists() {
                test_paths.push(path);
            } else {
                for dir_entry in std::fs::read_dir(&path).unwrap() {
                    let path = dir_entry.unwrap().path();
                    if path.join("===").exists() {
                        test_paths.push(path);
                    }
                }
            }
        }
    }
    test_paths.sort_unstable();
    let mut pass = 0;
    let mut fail = 0;
    let mut fail_list: Vec<(String, String)> = Vec::new();

    for test_path in test_paths.into_iter() {
        let test_path_str = test_path
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();
        if !SUPPORTED_TESTS.iter().any(|t| {
            test_path_str.as_str() == *t
                || test_path_str.starts_with(&format!("{}/", t))
        }) {
            continue;
        }
        let input_yaml = read_file(&test_path.join("in.yaml"));
        let expected_events = read_file(&test_path.join("test.event"));
        let is_error = test_path.join("error").exists();
        let result = YamlParser::parse_to_events(&input_yaml);
        let ok = if is_error {
            result.is_err()
        } else {
            match &result {
                Ok(events) => {
                    let mut events_str = String::new();
                    for event in events {
                        events_str.push_str(&event.to_string());
                        events_str.push('\n');
                    }
                    events_str == expected_events
                }
                Err(_) => false,
            }
        };
        if ok {
            pass += 1;
        } else {
            fail += 1;
            let msg = match &result {
                Err(e) => format!("ERR {e:?}"),
                Ok(events) => {
                    let mut events_str = String::new();
                    for event in events {
                        events_str.push_str(&event.to_string());
                        events_str.push('\n');
                    }
                    format!(
                        "EVENTS DIFFER\nexpected:\n{expected_events}\ngot:\n{events_str}"
                    )
                }
            };
            fail_list.push((test_path_str, msg));
        }
    }
    println!("==== PASS {pass} FAIL {fail} ====");
    for (name, msg) in &fail_list {
        println!("--- {name}\n{msg}");
    }
    assert!(fail == 0, "{} tests failing", fail);
}

fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}
