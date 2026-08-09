#[test]
fn iso() {
    let cases: &[(&str, &str)] = &[
        ("004s", "- -\n"),
        ("006s", "? -\n"),
        ("008s", "? key:\n"),
        ("010s", "- -1\n"),
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
