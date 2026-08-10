# Fuzz seed corpus

This directory is a small, git-tracked set of seed inputs for the
`from_str` and `from_reader` fuzz targets (see `../fuzz_targets/`).

It is intentionally separate from `fuzz/corpus/`, which is where
`cargo fuzz run` writes newly discovered, coverage-increasing inputs.
`fuzz/corpus/` is listed in `.gitignore` (it is large and
machine-generated), so seeds meant to be shared with every
contributor and CI run have to live here instead.

## Why these shapes

Coverage-guided mutation is unlikely to *discover* deeply nested
collections or long alias chains from scratch within a short time
budget - those shapes require many correlated byte changes at once,
which is exactly what mutation-based fuzzing struggles with. Each
file here gives the fuzzer a head start from a shape close to a
resource-limit boundary, so mutation explores *around* the boundary
instead of searching for it blindly. The shapes mirror the
deterministic regression tests in `../../src/tests/fuzz.rs`:

* `flow_seq_nested_*` / `block_seq_nested_*` - flow (`[[[...]]]`) and
  block (`- - - ...`) sequences nested at 100/127 (just under the
  128-level default depth limit), 200 and 5000 (over it).
* `block_map_nested_*` - the same idea for block mappings.
* `alias_chain_*` - chained anchors where each level aliases the
  previous one several times, so the composed node count grows
  combinatorially with depth (the "billion laughs" shape guarded by
  the node-budget check).
* `alias_flat_wide` - one anchor aliased many times from distinct
  keys (linear growth), a different corner of the same node-budget
  check.
* `mixed_nested_map_with_alias_chain` /
  `tagged_deeply_nested_flow_seq` - combined shapes (nesting plus
  aliasing, nesting plus a tag) for structural diversity.

## Usage

`cargo fuzz run` treats every positional directory argument after the
target name as a corpus directory to read from, and writes newly
discovered inputs into the *first* one given. Always pass the
writable corpus directory first and this seed directory second, for
example:

```sh
mkdir -p fuzz/corpus/from_str
cargo fuzz run from_str fuzz/corpus/from_str fuzz/seeds \
  -- -max_total_time=60
```

Passing only `fuzz/seeds` (with no writable corpus directory first)
would cause the fuzzer to write newly minimized inputs directly into
this directory, which is not what we want for a curated, git-tracked
seed set.
