# PLAN

Design and workflow roadmap. Snapshot reviewed: `c969e98` (v0.2.0).
`TODO.md` stays the short-term backlog; an item moves there when it is
scheduled. Completed items are removed from both.

**Status (2026-10-31):** D1 and the document-level part of D2 are
implemented: `Deserializer`/`DocumentDeserializer`, direct event
deserialization, `from_value`, `to_writer` and the multi-document
`Serializer`. `Deserializer::from_reader` exists but buffers the reader
in full. Still open: byte-level incremental parsing and borrowed
deserialization.

## Current state

The crate has a solid foundation: no `unsafe`, three runtime
dependencies, 190 passing tests, a clean `cargo fmt`/`cargo clippy
--all-targets`, yaml-test-suite event coverage, `error`-case coverage,
deterministic fuzz smoke tests, and resource limits for depth, node
count and reader size.

The processing pipeline is:

```
scanner -> parser -> Vec<YamlEvent> -> Value tree -> serde Deserializer
                                     \
                                      -> Value::to_string (dump)
```

The main structural issues are:

* `from_str` materializes the whole event stream, then a whole `Value`
  tree, then clones subtrees again while deserializing.
* sequence/map access clones each collection before walking it
  (`src/deserializer.rs:556`, `:560`, `:619`, `:661`), so nested
  documents are copied repeatedly.
* the parser and serializer both encode many presentation rules as
  ad-hoc string/line heuristics; the recent commit history is mostly
  fixes to those corners.
* the public API targets "drop-in replacement for `serde_yaml`", but
  several compatibility surfaces are missing or differ.

## Design improvements

### D1. Remove deserialization clones (P0, low risk)

Evidence: `deserialize_seq`, `deserialize_map` and `deserialize_enum`
clone the current node because the trait takes `&mut self`; the TODO
comments already call this out. `SequenceAccess`/`MappingAccess` then
pop owned children, so the first clone is pure waste and repeated at
every nesting level.

Change:

* Store the composed node as `Option<Value>` (or use
  `std::mem::take` on the `ValueData`) and move it into the access
  object instead of cloning.
* For tagged variants, move `tag.data` out the same way.
* Keep the current path/position bookkeeping.

Acceptance:

* no `to_vec()`/`*v.clone()` in `src/deserializer.rs`;
* existing deserializer tests pass unchanged;
* add a benchmark or allocation-count test showing a large nested
  document is no longer duplicated per level.

### D2. Make the event pipeline lazy and streaming (P1, unblocks TODO)

Today `YamlParser::parse_to_events_with_max_depth` returns
`Vec<YamlEvent>` (`src/parser.rs:14`, `:259`). Streaming and
multi-document support need an event source that yields per document
without composing a `Value`.

Suggested order:

1. Add a direct serde deserializer over the event stream (no `Value`
   intermediate). This delivers TODO item "streaming Deserializer"
   at the event level while keeping the parser intact.
2. Add `Deserializer::from_str`/`from_reader` as an iterator over
   documents, scoping anchors/tag handles per document. This covers
   the first two TODO items and reuses `YamlEventIter`.
3. Only then make the parser itself pull-based/streaming. The parser
   currently drains events to wrap flow nodes and attach properties
   (`take_events_since`, `attach_props_to_compact_key`), so this step
   must define a small event-sink/window abstraction instead of a
   plain `Vec`.

Acceptance:

* a document is deserialized without constructing a `Value` tree;
* `Deserializer` can iterate a multi-document stream lazily, with
  anchors and `%TAG` handles reset per document;
* peak memory for a one-document stream is O(input), not
  O(input + events + tree + clones);
* `TODO.md` items 1 and 2 can be removed.

### D3. Unify presentation logic in one emitter (P1)

There are two independent dump paths: the serde
`YamlSerializer` (`src/serializer.rs`) and `Value::to_string`
(`serialize_yaml_value_ctx`). They already disagree
(`max_width` is ignored by the `Value` dump), and both make layout
decisions by inspecting `self.output.ends_with(...)`, which is
brittle.

Change:

* extract a line-oriented emitter that tracks `line_start`,
  `column`, `indent`, and pending prefixes (`- `, `? `, `: `, `!tag `)
  explicitly;
* drive both the serde serializer and the `Value` dump through it;
* keep the current output conventions where tests depend on them, but
  stop adding `ends_with` special cases.

Acceptance:

* `to_string(&value)` and `value.to_string()` produce the same text
  for the same options;
* `max_width` is honored or documented consistently in both paths;
* golden suite tests and serializer tests still pass;
* add property tests: `parse(dump(v)) == v` and `dump(parse(s))` is
  idempotent over the suite corpus and generated values.

### D4. Decide and enforce `serde_yaml` compatibility (P1)

Missing or divergent surfaces found during review:

* no `from_value`, `to_writer`, indexed `Value` access
  (`value["key"]`, `value[0]`), `get_mut`, `as_sequence_mut`,
  `as_mapping_mut`, `Sequence`/`Number` aliases, or `TaggedValue` API;
* `Value` methods return `Result` where `serde_yaml` returns
  `Option` (e.g. `as_bool`, `as_str`);
* `from_str` is generic over `Deserialize<'a>` but cannot actually
  lend input data, because scalars are owned `String`s in the event
  stream. Borrowed targets (`&str`, borrowed `Cow`) therefore cannot
  work; decide whether to support them (likely requires `Cow<'de, str>`
  events) or document the limitation;
* `ValueData::Tag` serializes only the tag name
  (`collect_str(&tag.name)`, `src/value.rs:806`), losing the tagged
  content when a `Value` is passed through `rmsd_yaml::to_string`;
* `to_value` serializes to YAML text and re-parses it
  (`src/deserializer.rs:194`) instead of building a `Value` directly;
  this loses tag/anchor information and pays a full text round-trip;
* `Value::to_string()` (inherent) returns an empty string for a null
  document, while `rmsd_yaml::to_string(&Value::Null)` returns
  `null\n`. The name collision is a trap for drop-in users.

Change:

* write a compatibility matrix in `README.md` (supported / different /
  unsupported) against `serde_yaml`;
* add `from_value`, `to_writer`, indexed access and mutable accessors;
* implement a native `Value` serializer for `to_value` and fix tagged
  `Value` serialization;
* either implement borrowed deserialization or state explicitly that
  only `DeserializeOwned` semantics are supported;
* make this a checklist before the next minor release.

### D5. Keep the parser maintainable (P2)

The line-based lookahead is the source of most recent fixes. Do not
rewrite it blindly, but reduce the cost of each future corner case:

* restore `DESIGNS.md` (deleted in `efe1bdb`, recoverable with
  `git show efe1bdb^:DESIGNS.md`) and keep it current; it is still
  referenced by `src/tests/yaml_test_suite.rs:27`. Document the event
  model, the scanner/parser invariants, and every heuristic layer
  (`quoted_scalar_is_key`, `flow_collection_is_key`,
  `find_key_value_separator`, `strip_inline_comment`,
  `tab_content_is_block_node`, ...);
* add table-driven unit tests for each structural helper, including
  comments, quotes, tabs and flow indicators; most helpers are only
  exercised indirectly through the full suite;
* consider a `LineKind`/token classification step so the parser asks
  one place "what starts this line?" instead of repeating string
  checks;
* keep a known-limitations list next to the skip lists with one
  category per root cause (multi-document, YAML 1.3 fixtures,
  libyaml-specific `out.yaml` conventions, rejected tabs, ...).

## Workflow improvements

### W1. yaml-test-suite provenance and skip-list hygiene

The suite is vendored as 3,388 Git-tracked files under
`yaml-test-suit-data/` (note the directory spelling), with no record
of the upstream commit and no sync script. The skip lists also go
silently stale once a case starts passing.

* add `scripts/sync-yaml-test-suite.sh` that pins the upstream commit
  and regenerates the tree; mention the pin in `CHANGELOG` or a
  `yaml-test-suit-data/VERSION` file;
* add a test asserting each `SKIPPED_OUT_YAML_TEST` /
  `SKIPPED_IN_JSON_TEST` entry still exists and still fails for its
  documented reason; when a case is fixed the test fails and forces
  removal from the list;
* split the skip lists by reason and assert the reason at runtime
  where practical (e.g. "compose rejects multi-doc");
* turn `suite_probe` into an ignored diagnostic test or a small
  `examples/`/`xtask` binary; it duplicates the event test and runs on
  every `cargo test`;
* reuse one discovery helper instead of the three copies of
  `read_dir`/`===` walking.

### W2. CI and release

Current CI is useful but thin: stable `cargo test`, nightly
fmt/clippy, fuzz smoke, license. Add:

* an MSRV job (`cargo +1.88 check --all-targets`) alongside stable;
* `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"`;
* `cargo package --allow-dirty` / `cargo publish --dry-run` to catch
  packaging and metadata errors;
* run clippy/fmt on the same pinned toolchain as release, and pass
  `--locked` if the lockfile policy changes;
* decide the lockfile policy: `Cargo.lock` is currently gitignored,
  so CI resolves latest dependencies. For reproducible test runs,
  commit the lockfile and use `--locked`; otherwise add a scheduled
  `cargo update` job and keep the MSRV job honest;
* cache the cargo registry/target and move checkout to `actions/checkout@v4`;
* add a tag-triggered release job (test, dry-run publish, GitHub
  release from the matching `CHANGELOG` section), or adopt
  `release-plz`.

### W3. Test strategy

Keep the current suite tests, and add the following in priority order:

1. clone-free deserializer test/benchmark (D1);
2. property tests for `Value` dump/parse round-trip and idempotence
   (D3);
3. differential tests against `serde_yaml` for the public API subset
   and for error acceptance/rejection, feature-gated as a dev
   dependency while the replacement goal is active;
4. fuzz targets for `Value` dump -> parse and for
   `from_str` -> `to_string`, in addition to the current panic-only
   targets;
5. explicit tests for borrowed deserialization if it is supported, or
   a compile-fail/doc note if it is not.

### W4. Repository and API hygiene

Small, low-risk cleanups found in the review:

* `src/graph.rs` is not declared in `lib.rs`, references a
  non-existent `YamlTree`, and contains `todo!()`; delete it;
* `YamlPosition::add_by_str`/`pre_column` are unused; `add_by_str`
  also counts bytes and mishandles the final column, so remove or fix
  before use;
* `ErrorKind::FromStr` only parses `"bug"` (`src/error.rs:162`); it is
  a public footgun. Implement all variants, or remove it before 1.0;
* map I/O and invalid-UTF-8 failures to a dedicated
  `ErrorKind::Io`/`InvalidUtf8` instead of `ErrorKind::Bug`
  (`src/error.rs:267`; `src/tests/fuzz.rs` currently codifies `Bug`);
* add `#[non_exhaustive]` to `ErrorKind` and `ValueData` before 1.0,
  and fix the `InvalidSequnceStartIndicator` /
  `no_support_mulitple_documents` spellings while breaking changes are
  still cheap;
* fix `Cargo.toml`'s `repository` URL (`nispor/rdsm-yaml` typo) and
  align `README.md`'s "YAML 1.2" with "YAML 1.2.2";
* `README.md` promises `cargo bench` but there is no `benches/`
  directory: add the benchmark and a regression budget, or remove the
  claim;
* document the resource limits (`YamlParseOption`) and the
  `Value::to_string` vs `rmsd_yaml::to_string` difference in the
  README.

## Suggested order

1. **Housekeeping:** W4 + W2 (MSRV/docs/package jobs), remove dead
   code, fix metadata.
2. **D1:** clone-free deserialization and its benchmark.
3. **D2 (step 1-2):** event-stream deserializer and multi-document
   `Deserializer` iterator; remove TODO items 1-2.
4. **D3:** emitter refactor plus round-trip property tests.
5. **D4:** compatibility matrix and missing public API.
6. **D2 (step 3), D5:** truly lazy parser and parser documentation
   only if the streaming requirement or maintenance cost justifies it.

## Decisions needed

* Is `Cargo.lock` committed for reproducible CI?
* What exactly does "drop-in replacement" mean: compile-compatible
  for common APIs, or semver/API-compatible with `serde_yaml`?
* Should borrowed deserialization (`Deserialize<'de>` yielding
  `&str`) be supported, and therefore should events borrow from the
  input?
* Are aliases represented as duplicated subtrees (current behavior,
  with the node budget), or should an internal arena/node graph keep
  alias expansion O(1) while `Value` materializes only on demand?
* Do we keep the current "skip known-unfixable fixtures" policy, or
  move to expected-failure tests with explicit reasons?
