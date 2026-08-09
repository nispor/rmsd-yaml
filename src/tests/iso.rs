#[test]
fn iso() {
    super::testlib::init_logger();
    let input = "sequence: !!seq\n- entry\n- !!seq\n - nested\nmapping: !!map\n foo: bar\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e}"); } }
        Err(e) => println!("ERR {e:?}"),
    }
}
