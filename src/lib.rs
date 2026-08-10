// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

//! A pure-Rust YAML serializer and deserializer implementing the
//! [`serde`](https://serde.rs) traits, designed as a drop-in
//! replacement for `serde_yaml` with minimal dependencies.
//!
//! # Examples
//!
//! ```
//! use rmsd_yaml::{from_str, to_string};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Config {
//!     name: String,
//!     retries: u32,
//! }
//!
//! let config = Config {
//!     name: "alpha".into(),
//!     retries: 3,
//! };
//!
//! let yaml = to_string(&config)?;
//! assert_eq!(yaml, "name: alpha\nretries: 3\n");
//!
//! let back: Config = from_str(&yaml)?;
//! assert_eq!(back, config);
//! # Ok::<(), rmsd_yaml::Error>(())
//! ```
//!
//! Multiple documents, tags, anchors and block scalars are supported:
//!
//! ```
//! use rmsd_yaml::{from_str, Value};
//!
//! let value: Value = from_str("- &a\n  text\n- *a\n")?;
//! # Ok::<(), rmsd_yaml::Error>(())
//! ```

mod anchor;
mod base64;
pub(crate) mod compose;
mod deserializer;
mod error;
mod event;
mod map;
mod parser;
mod position;
mod scalar;
mod scalar_ser;
mod scanner;
mod sequence;
mod serializer;
mod state;
mod tag;
mod value;
mod variant;

#[cfg(test)]
mod tests;

pub use self::{
    deserializer::{
        YamlDeserializer, YamlParseOption, documents, documents_with_opt,
        from_reader, from_reader_with_opt, from_str, from_str_with_opt,
        to_value,
    },
    error::{Error, ErrorKind},
    event::YamlScalarStyle,
    map::Mapping,
    position::YamlPosition,
    serializer::{
        YamlSerializeOption, YamlSerializer, to_string, to_string_with_opt,
    },
    value::{Value, ValueData, ValueMeta},
};
pub(crate) use self::{
    event::{YamlCollectionStyle, YamlEvent, YamlEventIter},
    map::MappingAccess,
    parser::YamlParser,
    scalar_ser::{
        escape_double_quoted, to_out_yaml_scalar_plain, to_out_yaml_scalar_sq,
        to_scalar_string,
    },
    scanner::YamlScanner,
    sequence::SequenceAccess,
    state::YamlState,
    tag::YamlTag,
    variant::ValueEnumAccess,
};
