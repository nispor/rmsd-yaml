# RMSD-YAML Development Plan

## Overview

RMSD-YAML is a pure Rust, minimised YAML library targeting serde compatibility and yaml-test-suite conformance as a replacement for `serde_yaml`.

## Current Status

* Core parser/scanner infrastructure exists (YAML spec 1.2.2)
* Compose phase converts events to YamlValue tree
* Deserializer/Serializer implement serde traits
* 196 of 402 yaml-test-suite cases enabled and passing
* 54 cargo unit tests pass (serializer, deserializer, scalar, base64)
* Anchors and aliases are parsed and resolved in block and flow
  contexts; flow sequences/mappings (incl. nested and single-pair
  forms) are supported
* Block scalars (`|`/`>`) support chomping, explicit indentation
  indicators, leading/trailing empty lines and line folding
* Missing: explicit `?` block keys, tag/directive handling, tag
  shorthands for secondary handles

## Milestones

### M1 - Parser & Compose Core (DONE)

**Goal:** Pass 20+ yaml-test-suite cases for core mapping/sequence
patterns. Reached: 109/402 cases pass.

Tasks:
* [x] Fix/implement anchor handling in parser (new `anchor.rs` with
      `handle_anchor()`; `YamlEvent` carries the anchor; the
      `graph.rs` `todo!()` is orphaned code, not part of the build)
* [x] Complete implicit key value handling (removed the dead
      `_spliter_offset` line in `map.rs`)
* [x] Enable `"anchors-in-mapping"` test case (this plan previously
      said `"anchors-in-block-mapping"`, which does not exist in the
      suite data)
* [x] Enable `"anchor-and-alias-as-mapping-key"` test case
* [x] Fix sequence completion (fixed an empty-line hang in
      `handle_block_seq`; the `sequence.rs` `todo!()` is
      `handle_flow_seq`, which belongs to M3)
* [x] Enable `"single-entry-block-sequence"` test case
* [x] Handle document end markers properly (`handle_node` stops at
      `...`; a lone `...` stream emits only `+STR -STR`)
* [x] Enable `"missing-document-end-marker-before-directive"` test
      case (this plan previously said
      `"missing-document-footer-before-directives"`)

Extra work done to make the enabled cases pass:
* [x] Directive position check: `%` lines outside document boundaries
      raise `MissingDocumentEndMarkerBeforeDirective` (full directive
      support stays in M6)
* [x] Node property loop guards against a second anchor/tag on one
      node (was an infinite recursion)
* [x] `YamlScalarStyle` added to `YamlEvent::Scalar`; the suite event
      format encodes presentation style, not content
* [x] Block scalar chomping indicators were swapped: `+` is keep and
      `-` is strip
* [x] Block scalar indentation indicator is relative to the parent
      node indent, not the sequence entry content column
* [x] Local tags render as `<!name>`; bare `!` emits `<!>` on an
      empty scalar
* [x] Comments after plain scalar values are stripped
* [x] Enabled every suite case that currently passes (100 top-level
      names, 109 counted cases)

### M2 - Deserializer Completeness (DONE)

**Goal:** Full serde `Deserializer` trait implementation that matches
serde_yaml behavior. Reached: 118/402 cases pass.

Tasks:
* [x] Replace `todo!()` stubs in `deserializer.rs` (`deserialize_char`,
      the enum methods and `deserialize_newtype_struct` were already
      implemented; the remaining stubs were `deserialize_f32/f64`,
      `deserialize_unit`, `deserialize_unit_struct`, `deserialize_bytes`
      and `deserialize_byte_buf`)
* [x] Match serde_yaml: `deserialize_f32` delegates to f64; bytes and
      byte_buf are unsupported; floats accept `.inf`/`.nan`
* [x] Handle anchor reference resolution in compose phase (anchor
      table; aliases resolve to a clone of the anchored node; unknown
      alias raises `UnknownAlias`)
* [x] Implement proper alias node handling in `YamlValueEnumAccess`
      (aliases resolve in compose; enum variant names are unwrapped
      from `<!Variant>`/`<tag:...:Variant>` tags)
* [x] Add type coercion tests for bool/char/int/float/string/unit/
      option/bytes/enum edge cases

Extra work done to make the enabled cases pass:
* [x] Parser: `Alias` event (`=ALI *name`), alias as mapping key
      (`*b : *a`), anchored empty values (`a: &anchor\nb: *anchor`)
* [x] Fix latent parser state bug: `pop_state()` only popped the state
      when trace logging was enabled (side effect inside `log!`)
* [x] Enable every suite case that currently passes (109 top-level
      names, 118 counted cases)

### M3 - Flow Collections + Anchors + Aliases (DONE)

**Goal:** Full flow collection and anchor/alias coverage. Reached:
175/402 cases pass.

Tasks:
* [x] Implement flow sequence parsing (`[item1, item2]`), including
      single-pair mappings inside a sequence (`[ a: b ]`)
* [x] Implement flow mapping parsing (`{key: val}`), including
      empty keys/values and explicit `?` flow keys
* [x] Track node anchors in compose and resolve aliases by reference
      (done in M2)
* [x] Handle nested flow collections (`[[1, 2], {a: [3]}]`) and flow
      collections inside block context, including as mapping keys
* [x] Anchor duplication: a second anchor on the same node is
      rejected; redefining an anchor for a later node is allowed
      (YAML 1.2.2 behavior)
* [x] Single-quoted flow scalars
* [x] Enable every passing `anchors-*`, `aliases-*` and flow test
      case

Extra work done to make the enabled cases pass:
* [x] Nested block sequences on one line (`- - - []`)
* [x] Reject a block sequence entry on the same line as a mapping key
      (`key: - a`) and adjacent content after a flow collection
      (`{a: b}c`)

Remaining anchor/alias cases need explicit `?` block-key support:
`aliases-in-explicit-block-mapping`, `anchors-on-empty-scalars`,
`key-with-anchor-after-missing-explicit-mapping-value`.

### M4 - Block Scalars & Chomping (DONE)

**Goal:** Full literal/folded block scalar support with chomping control.
Reached: 196/402 cases pass.

Tasks:
- [x] Implement block scalar indicators (`|`, `>`, `|-`, `->`, etc.)
- [x] Handle indentation-based content extraction (content indent
      relative to the enclosing block collection; explicit indentation
      indicator `1`-`9`, `0` rejected)
- [x] Support zero-indented block scalars (top-level nodes may have
      content at indent 0)
- [x] Handle more-indented lines in folded mode (kept verbatim, never
      folded)
- [x] Enable `"literal-scalar"`, `"folded-scalar"`, `"block-scalar-*"`
      tests (24+ tests)
- [x] Fix scalar escaping (`scalar_ser.rs` now double-quotes strings
      needing escapes and escapes non-printable characters)

Extra work done to make the enabled cases pass:
- [x] Rewrite both block scalar handlers into one spec-compliant
      implementation (leading empty lines are content once a content
      line exists, trailing empty lines follow chomping, folding turns
      line breaks into spaces only between two regular lines)
- [x] Reject leading empty lines more indented than the first
      non-empty line and a tab inside the indentation region
- [x] `expect_comment_or_line_break` stops after a comment and
      requires a space before the `#` (block scalar header comments)
- [x] Comment lines are skipped at stream, mapping and sequence level;
      `...` ends block maps/sequences without being consumed
- [x] Node properties on their own line re-derive the content
      indentation (e.g. `!foo` before `>1`)
- [x] Event DSL escapes `\`, `\t` in addition to `\n`

### M5 - Serializer Improvements ✅ DONE

**Goal:** Complete serde `Serializer` trait and produce round-trippable output.

Tasks:
- [x] Replace all `todo!()` in `serializer.rs`:
  - `serialize_tuple_variant` / `serialize_struct_variant` render
    `!Variant` + block sequence / mapping (matching serde_yaml)
  - `SerializeTupleVariant` / `SerializeStructVariant` field methods
    implemented
- [x] Implement base64 encoding/decoding for `!!binary` tags
  (new `src/base64.rs`, RFC 4648, dependency-free)
- [x] `serialize_bytes` emits `!!binary <base64>`; `deserialize_bytes` /
  `deserialize_byte_buf` decode it (CString & `&[u8]` visitors)
- [x] Fix long-line breaking in `scalar_ser.rs` (folded double-quoted
  output round-trips; done in M4, covered by serializer tests)
- [x] Add indentation validation tests (`indent_count < 2` rejected)
- [x] Fix `serialize_unit_struct` -> `null` and `serialize_newtype_struct`
  passthrough (previously wrote wrong `!name null` / `!name` lines)
- [x] Fix tuple / tuple-struct elements missing the trailing newline
- [x] Fix `pending_tag` leaking into following collections after a
  tagged scalar (cleared on every scalar write)
- [x] Fix nested block map state leak: the map loop now restores its own
  `InBlockMapKey` state after each value, so sibling keys are no longer
  swallowed into nested maps (`a:\n  b: 1\n  c: 2` parses correctly)
- [x] Fix double-quoted scalar escapes: escaped `\n` is content and no
  longer folded to a space by flow folding (read-time folding only
  applies to real line breaks)
- [x] `deserialize_option` / `deserialize_unit` treat `null`, `~` and
  empty scalars as None/unit (Core Schema)
- [x] Serializer test module `src/tests/serializer.rs`: round-trips for
  seqs/maps/enums/structs/options, binary tags, long-line folding,
  leading `---`, indentation validation

### M6 - Tags & Directives

**Goal:** Full YAML directive and tag handling.

Tasks:
- [ ] Implement `tag.rs` logic for primary/secondary/hex/verbatim handles
- [ ] Handle global tags (`!!str`, `<!tag:...>`)
- [ ] Implement explicit vs implicit tag resolution
- [ ] Add `%TAG` directive parsing and validation
- [ ] Enable `"global-tags"`, `"tag-shorthands"`, `"directive"` tests (8+ tests)

### M7 - Edge Cases & Compliance

**Goal:** Achieve 100% yaml-test-suite conformance.

Tasks:
- [ ] Handle all edge cases identified in disabled test list:
  - Bare documents, stream parsing
  - Escape sequences in double-quoted scalars
  - Unicode handling (NFC/NFD normalization)
  - Tab/space indentation conflicts
  - Line-length limits and max_width enforcement
- [ ] Add error validation for invalid YAML (ensure proper `Error` type coverage)
- [ ] Implement all remaining deserializer methods
- [ ] Run full test suite, fix failures

### M8 - Performance & Polish

**Goal:** Optimize performance vs serde_yaml and prepare for production use.

Tasks:
- [ ] Profile against serde_yaml on benchmark datasets
- [ ] Eliminate unnecessary clones in compose phase
- [ ] Implement streaming deserializer (like `Deserializer::deserialize_...` with iterators)
- [ ] Add benchmarks to Cargo.toml as `[dev-dependency]` criterion
- [ ] Add documentation with examples
- [ ] Consider removing serde dependency entirely for direct API option

## Disabled Test Cases Reference

Current enabled tests: see `supported_tests` in
`src/tests/yaml_test_suite.rs` (183 top-level names, 196 counted
cases). That list is the source of truth.

Disabled categories (priority order):
1. Tags/directives (~10 tests left) - M6
2. Explicit `?` block keys (~5 tests) - M6/M7
3. Edge cases (~40+ tests) - M7
