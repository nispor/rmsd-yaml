# rmsd_yaml

A pure-Rust YAML serializer and deserializer implementing the
[`serde`](https://serde.rs) traits, designed as a drop-in replacement
for `serde_yaml` with minimal dependencies (`serde`, `indexmap`, `log`).

## Features

- Parse YAML 1.2 documents (block and flow collections, block scalars
  with chomping, anchors/aliases, tags and directives, multi-document
  streams)
- Serialize any `Serialize` type to YAML with round-trippable output
  (enums as `!Variant` tags, `!!binary` for byte buffers)
- yaml-test-suite conformance: **251 of 402** cases passing and growing
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

## Serialization options

```rust
use rmsd_yaml::{to_string_with_opt, YamlSerializeOption};

let yaml = to_string_with_opt(
    &vec![1, 2],
    YamlSerializeOption {
        leading_start_indicator: true, // emit a leading `---`
        indent_count: 2,
        max_width: 80,
    },
)?;
# Ok::<(), rmsd_yaml::Error>(())
```

## Performance

Run `cargo bench` for a comparison against `serde_yaml` on a
representative configuration document (parse and serialize throughput).

## License

Apache-2.0
