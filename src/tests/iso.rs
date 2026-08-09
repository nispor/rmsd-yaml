#[test]
fn iso() {
    super::testlib::init_logger();
    let input = "key: &x\n!!map\n  a: b\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e}"); } }
        Err(e) => println!("ERR {e:?}"),
    }
}
