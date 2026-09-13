- Divergence from libyaml (stricter, spec-supported): a lone `\r`
  line break inside a double-quoted scalar in block context is
  rejected by the continuation-line indentation rule (production
  [197]), while libyaml folds `k: "a\rb"` to `a b`.
- Streaming `Deserializer` (incremental, without the intermediate
  `YamlValue` tree)
- Multi-document stream parsing (a lazy, per-document iterator
  comparable to `serde_yaml::Deserializer`'s `Iterator` impl).
  The earlier eager `documents`/`documents_with_opt` API was
  removed since it did not align with `serde_yaml`'s API shape;
  revisit only on request.
- Support multi-document streams in the dump
  (`NoSupportMultipleDocuments` failures)
