# TODO

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
