#[test]
fn iso() {
    super::testlib::init_logger();
    let input = "plain key: in-line value\n: # Both empty\n\"quoted key\":\n- entry\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e}"); } }
        Err(e) => println!("ERR {e:?}"),
    }
}
