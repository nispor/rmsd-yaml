#[test]
fn iso() {
    super::testlib::init_logger();
    let input = "control: \"\\b1998\\t1999\\t2000\\n\"\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e:?}"); } }
        Err(e) => println!("ERR {e:?}"),
    }
}
