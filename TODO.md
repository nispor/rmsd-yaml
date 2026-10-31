- Truly incremental single-document parsing: the parser currently
  buffers one document's events, so memory is O(largest document).
- Truly streaming reader input: `Deserializer::from_reader` buffers the
  whole reader before parsing (the document iterator itself is lazy).
- Borrowed deserialization (`T: Deserialize<'de>` yielding `&str`)
  requires borrowing scalar slices from the input; scalars are owned
  `String`s today.
