# rmsd_yaml

A pure-Rust YAML serializer and deserializer implementing the
[`serde`](https://serde.rs) traits, designed as a drop-in replacement
for `serde_yaml` with minimal dependencies (`serde`, `indexmap`, `log`).

## Features

- YAML 1.2.2 support
- API similar to serde-yaml
- Multi-document stream parsing and dumping
- No `unsafe` code; no C bindings

## Quick start

```rust
use rmsd_yaml::{from_str, to_string};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Config {
    name: String,
    retries: u32,
    tags: Vec<String>,
}

let config = Config {
    name: "alpha".into(),
    retries: 3,
    tags: vec!["fast".into(), "stable".into()],
};

// Serialize.
let yaml = to_string(&config)?;
assert_eq!(
    yaml,
    "name: alpha\nretries: 3\ntags:\n  - fast\n  - stable\n"
);

// Deserialize.
let back: Config = from_str(&yaml)?;
assert_eq!(back, config);
# Ok::<(), rmsd_yaml::Error>(())
```

## Working with the value model

`Value` mirrors the YAML document tree, including tags:

```rust
use rmsd_yaml::{from_str, ValueData};

let value = from_str::<Value>("a: 1\nb: [x, y]\n")?;
match &value.data {
    ValueData::Map(map) => {
        assert!(map.contains_key(&"a".into()));
    }
    _ => unreachable!(),
}
# Ok::<(), rmsd_yaml::Error>(())
```

## Multi-document streams

`Deserializer` iterates the documents of a stream lazily. Each item is
a single-document deserializer:

```rust
use rmsd_yaml::{Deserializer, Value};
use serde::Deserialize;

let input = "---\nname: first\n---\nname: second\n";
for document in Deserializer::from_str(input) {
    let value = Value::deserialize(document)?;
    println!("{value:?}");
}
# Ok::<(), rmsd_yaml::Error>(())
```

Dumping multiple documents into one stream uses `Serializer`; the
first document follows the normal options and later documents are
separated with `---`:

```rust
use rmsd_yaml::Serializer;

let mut serializer = Serializer::new(Vec::new());
serializer.serialize(&vec![1, 2])?;
serializer.serialize(&vec![3])?;
assert_eq!(serializer.into_inner(), b"- 1\n- 2\n---\n- 3\n".to_vec());
# Ok::<(), rmsd_yaml::Error>(())
```

`from_str` and `from_reader` remain single-document APIs and reject a
stream with more than one document.

## Serialization options

```rust
use rmsd_yaml::{to_string_with_opt, YamlSerializeOption};

let yaml = to_string_with_opt(
    &vec![1, 2],
    YamlSerializeOption {
        leading_start_indicator: true, // emit a leading `---`
        indent_count: 2,
        max_width: 80,
        ..Default::default()
    },
)?;
# Ok::<(), rmsd_yaml::Error>(())
```

## Limitations

* `from_str`/`from_reader` reject multi-document streams with
  `ErrorKind::NoSupportMultipleDocuments`; use `Deserializer` for
  those.
* Byte-identical round-trip is not promised. `Value::to_string()`
  re-emits parsed documents as faithfully as supported, but
  yaml-test-suite corner cases requiring extensive design changes
  are unsupported.
* Tagged YAML is not supported when deserializing into generic `any`
  targets without `visit_enum`. Tagged nodes are supported by `Value`
  and Rust enum targets.

## License

Apache-2.0
