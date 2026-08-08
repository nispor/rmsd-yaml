# RMSD-YAML Development Plan

## Overview

RMSD-YAML is a pure Rust, minimised YAML library targeting serde compatibility and yaml-test-suite conformance as a replacement for `serde_yaml`.

## Current Status

* Core parser/scanner infrastructure exists (YAML spec 1.2.2)
* Compose phase converts events to YamlValue tree
* Deserializer/Serializer implement serde traits
* 109 of 402 yaml-test-suite cases enabled and passing
* Anchors are parsed in block mapping/sequence contexts; alias
  resolution in compose is still missing
* Missing: flow collections, single-quoted scalars, tag/directive
  handling, alias events

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

### M2 - Deserializer Completeness

**Goal:** Full serde `Deserializer` trait implementation that matches serde_yaml behavior.

Tasks:
- [ ] Replace `todo!()` stubs in `deserializer.rs`:
  - `deserialize_char` (line ~140)
  - `deserialize_byte_buf` (line ~147)
  - All enum deserialization methods (lines ~200-230)
  - `deserialize_bytes`, `deserialize_seq_key`, `deserialize_newtype_struct`
- [ ] Handle anchor reference resolution in compose phase
- [ ] Implement proper alias node handling in `YamlValueEnumAccess`
- [ ] Add type coercion tests for bool/char/int/string edge cases

### M3 - Flow Collections + Anchors + Aliases

**Goal:** Full flow collection and anchor/alias coverage.

Tasks:
- [ ] Implement flow sequence parsing (`[item1, item2]`)
- [ ] Implement flow mapping parsing (`{key: val}`)
- [ ] Track node IDs for anchors, resolve aliases by reference
- [ ] Handle nested flow collections (`[[1, 2], {a: [3]}]`)
- [ ] Add anchor name deduplication validation
- [ ] Enable all `anchors-*` and `aliases-*` test cases (15+ tests)

### M4 - Block Scalars & Chomping

**Goal:** Full literal/folded block scalar support with chomping control.

Tasks:
- [ ] Implement block scalar indicators (`|`, `>`, `|-`, `->`, etc.)
- [ ] Handle indentation-based content extraction
- [ ] Support zero-indented block scalars
- [ ] Handle more-indented lines in folded mode
- [ ] Enable `"literal-scalar"`, `"folded-scalar"`, `"block-scalar-*"` tests (10+ tests)
- [ ] Fix scalar escaping (`scalar_ser.rs` TODOs for non-printable chars)

### M5 - Serializer Improvements

**Goal:** Complete serde `Serializer` trait and produce indistinguishable output from serde_yaml.

Tasks:
- [ ] Replace `todo!()` in serializer:
  - Anchor rendering (line ~306)
  - Tag handling (line ~292, ~195)
  - Various serialization methods (~431, ~435, ~514, ~518)
- [ ] Implement base64 encoding for binary tags
- [ ] Fix long-line breaking in scalar_ser.rs
- [ ] Add indentation validation (already exists but add tests)
- [ ] Support custom tag rendering

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
`src/tests/yaml_test_suite.rs` (100 top-level names, 109 counted
cases). That list is the source of truth.

Disabled categories (priority order):
1. Anchors/Aliases (~10 tests left) - M3
2. Flow collections (~10 tests) - M3
3. Block scalars/chomping (~8 tests left) - M4
4. Tags/directives (~6 tests left) - M6
5. Edge cases (~40+ tests) - M7
