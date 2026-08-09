# TODO

Open tasks extracted from `PLAN.md` (removed).

## M7 - Edge Cases & Compliance

* [ ] Explicit/empty keys in flow collections (`{ ? foo :, : bar }`,
      `{ foo : !!str }`)
* [ ] Explicit-key edge cases (`? : x`, `? []: x` nested structures)
* [ ] Unicode NFC/NFD normalization
* [ ] Line-length limits / max_width enforcement in the parser

## M8 - Performance & Polish

* [ ] Streaming `Deserializer` (incremental, without the intermediate
      `YamlValue` tree)

## out.yaml tests

* [ ] Pass all to_yaml tests: implement the event-to-YAML emitter so
      `yaml_test_suit_out_yaml` passes every case in
      `SUPPORTED_OUT_YAML_TEST` (currently empty; enable cases as the
      emitter matures)
