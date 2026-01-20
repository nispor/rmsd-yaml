// SPDX-License-Identifier: Apache-2.0

mod compose;
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
pub(crate) mod testlib;
#[cfg(test)]
mod yaml_test_suite;


pub use self::{
    deserializer::{YamlDeserializer, from_str, to_value},
    error::{ErrorKind, YamlError},
    map::YamlValueMap,
    position::YamlPosition,
    serializer::{
        YamlSerializeOption, YamlSerializer, to_string, to_string_with_opt,
    },
    value::{YamlValue, YamlValueData},
};
pub(crate) use self::{
    event::{YamlEvent, YamlEventIter},
    map::YamlValueMapAccess,
    parser::YamlParser,
    scalar_ser::to_scalar_string,
    scanner::YamlScanner,
    sequence::YamlValueSeqAccess,
    state::YamlState,
    tag::YamlTag,
    variant::YamlValueEnumAccess,
};
