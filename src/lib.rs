// SPDX-License-Identifier: Apache-2.0

mod array;
mod char_iter;
mod deserializer;
mod error;
mod indent;
mod map;
mod position;
mod scalar_str;
mod token;
mod token_iter;
mod value;
mod variant;

pub(crate) use self::array::{get_array, YamlValueSeqAccess};
pub(crate) use self::char_iter::CharsIter;
pub(crate) use self::indent::process_indent;
pub(crate) use self::map::{get_map, YamlValueMapAccess};
pub(crate) use self::scalar_str::{
    read_double_quoted_str, read_single_quoted_str, read_unquoted_str,
};
pub(crate) use self::token::{YamlToken, YamlTokenData, YAML_CHAR_INDICATORS};
pub(crate) use self::token_iter::TokensIter;
pub(crate) use self::variant::{get_tag, YamlValueEnumAccess};

pub use self::deserializer::{from_str, to_value, RmsdDeserializer};
pub use self::error::RmsdError;
pub use self::map::YamlValueMap;
pub use self::position::RmsdPosition;
pub use self::value::{YamlTag, YamlValue, YamlValueData};
