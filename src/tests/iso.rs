#[test]
fn iso() {
    super::testlib::init_logger();
    let input = "a:\n  b:\n    c: d\n  e:\n    f: g\nh: i\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e}"); } }
        Err(e) => println!("ERR {e:?}"),
    }
}
