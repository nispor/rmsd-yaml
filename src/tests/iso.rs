#[test]
fn iso() {
    let input = "{ &a [a, &b b]: *b, *a : [c, *b, d]}\n";
    match crate::YamlParser::parse_to_events(input) {
        Ok(v) => { for e in &v { println!("{e}"); } }
        Err(e) => println!("ERR {e}"),
    }
}
