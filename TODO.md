# TODO

## Edge Cases & Compliance

* [ ] Explicit/empty keys in flow collections (`{ ? foo :, : bar }`,
      `{ foo : !!str }`)
* [ ] Explicit-key edge cases (`? : x`, `? []: x` nested structures)
* [ ] Unicode NFC/NFD normalization
* [ ] Line-length limits / max_width enforcement in the parser

## Performance & Polish

* [ ] Streaming `Deserializer` (incremental, without the intermediate
      `YamlValue` tree)

## out.yaml tests

* [ ] Pass the remaining to_yaml tests: the value-based dump
      (`YamlValue::to_string()`) passes the 93 cases in
      `SUPPORTED_OUT_YAML_TEST`; the other 120 `out.yaml` files cannot
      be reproduced by a value dump (anchors/aliases, `---`/`...`
      markers, block-scalar and quoted styles, multi-document streams;
      see `DESIGNS.md`)

## in.json tests

* [ ] Enable in.json test suite: deserialize each `in.yaml` and compare
      the result with the JSON value in `in.json`
