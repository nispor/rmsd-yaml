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

* [x] Pass the to_yaml tests: the value-based dump
      (`YamlValue::to_string()`) passes the 201 cases in
      `SUPPORTED_OUT_YAML_TEST`; the other 48 `out.yaml` files cannot
      be reproduced (14 multi-document streams; 34 hand-authored
      deviations from the canonical events; see `DESIGNS.md`)
* [ ] Support multi-document streams in the dump (the remaining 14
      `NoSupportMultipleDocuments` failures)

## in.json tests

* [ ] Enable in.json test suite: deserialize each `in.yaml` and compare
      the result with the JSON value in `in.json`
