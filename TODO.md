# TODO

## Edge-case & compliance gaps

* [ ] Serde-path `Value` round-trip fidelity: `from_str::<Value>()`
      (the `Deserialize` impl) drops scalar style and explicit tags,
      so `a: ''` round-trips to `{a: null}` and `{ foo : !!str }`
      loses the tag. The compose path (`Value::from_str()` /
      `parse::<Value>()` -> `to_string()`) round-trips both correctly;
      the serde path should preserve the same metadata.
* [ ] Anchor placement on compact explicit keys: `? &a k : v` attaches
      `&a` to the compact single-pair mapping used as the key, while
      libyaml attaches it to the scalar `k` (an alias `*a` resolves to
      a different node).
* [ ] Divergence from libyaml (stricter, spec-supported): a lone `\r`
      line break inside a double-quoted scalar in block context is
      rejected by the continuation-line indentation rule (production
      [197]), while libyaml folds `k: "a\rb"` to `a b`.

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
