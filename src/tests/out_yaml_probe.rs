#[test]
fn out_yaml_probe() {
    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("yaml-test-suit-data/name");
    let mut test_paths = vec![];
    for entry in std::fs::read_dir(&test_data_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.join("===").exists() {
                test_paths.push(path);
            } else {
                for de in std::fs::read_dir(&path).unwrap() {
                    let p = de.unwrap().path();
                    if p.join("===").exists() {
                        test_paths.push(p);
                    }
                }
            }
        }
    }
    test_paths.sort_unstable();
    for tp in test_paths {
        let rel = tp
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();
        if !tp.join("out.yaml").exists() {
            continue;
        }
        let iny = std::fs::read_to_string(tp.join("in.yaml")).unwrap();
        let got = crate::to_value(&iny)
            .and_then(|v| v.to_string())
            .unwrap_or_default();
        println!("{rel}\t{got:?}");
    }
}
