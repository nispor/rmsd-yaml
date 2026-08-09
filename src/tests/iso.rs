#[test]
fn iso() {
    let cases: &[(&str, &str)] = &[
        ("flow_kc", "[key:]\n"),
        ("flow_dash", "[a, -]\n"),
        ("flow_colon", "[a, :]\n"),
    ];
    for (name, input) in cases {
        match crate::YamlParser::parse_to_events(input) {
            Ok(v) => {
                let mut s = String::new();
                for e in &v { s.push_str(&format!("{e}\n")); }
                println!("{name}: OK\n{s}");
            }
            Err(e) => println!("{name}: ERR {e:?}"),
        }
    }
}
