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
//! Tags, anchors and block scalars are supported.
//!
//! ```
//! use rmsd_yaml::{from_str, Value};
//!
//! let value: Value = from_str("- &a\n  text\n- *a\n")?;
//! # Ok::<(), rmsd_yaml::Error>(())
//! ```
//!
//! Multi-document streams are parsed with [`Deserializer`] and dumped
//! with [`Serializer`]. The single-document helpers ([`from_str`],
//! [`from_reader`], [`to_string`]) reject a stream with more than one
//! document.
//!
//! ```
//! use rmsd_yaml::{Deserializer, Value};
//! use serde::Deserialize;
//!
//! let mut documents = Deserializer::from_str("---\na: 1\n---\na: 2\n");
//! let first = Value::deserialize(documents.next().unwrap())?;
//! let second = Value::deserialize(documents.next().unwrap())?;
//! assert!(documents.next().is_none());
//! assert_eq!(first.get("a").unwrap().as_str()?, "1");
//! assert_eq!(second.get("a").unwrap().as_str()?, "2");
//! # Ok::<(), rmsd_yaml::Error>(())
//! ```

mod anchor;
mod base64;
pub(crate) mod compose;
mod deserializer;
mod error;
mod event;
pub(crate) mod event_de;
mod map;
mod parser;
mod position;
mod scalar;
mod scalar_ser;
mod scanner;
mod sequence;
mod serializer;
mod state;
mod stream;
mod tag;
mod value;
mod variant;

#[cfg(test)]
mod tests;

pub use self::{
    deserializer::{
        YamlDeserializer, YamlParseOption, from_reader, from_reader_with_opt,
        from_str, from_str_with_opt, from_value, to_value,
    },
    error::{Error, ErrorKind},
    event::YamlScalarStyle,
    map::Mapping,
    position::YamlPosition,
    serializer::{
        Serializer, YamlSerializeOption, YamlSerializer, to_string,
        to_string_with_opt, to_writer, to_writer_with_opt,
    },
    stream::{Deserializer, DocumentDeserializer},
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
