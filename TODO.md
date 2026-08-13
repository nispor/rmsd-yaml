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
* [ ] Multi-document stream parsing (a lazy, per-document iterator
      comparable to `serde_yaml::Deserializer`'s `Iterator` impl).
      The earlier eager `documents`/`documents_with_opt` API was
      removed since it did not align with `serde_yaml`'s API shape;
      revisit only on request.

## out.yaml tests

* [ ] Support multi-document streams in the dump (the remaining 14
      `NoSupportMultipleDocuments` failures)
