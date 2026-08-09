// SPDX-License-Identifier: Apache-2.0

//! Lightweight benchmark comparing `rmsd_yaml` with `serde_yaml` on
//! deserialization and serialization throughput.
//!
//! Run with: `cargo bench`

use std::hint::black_box;
use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Config {
    server: Server,
    clients: Vec<Client>,
    features: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Server {
    host: String,
    port: u16,
    workers: usize,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Client {
    name: String,
    retries: u32,
    tags: Vec<String>,
}

const YAML: &str = r#"
server:
  host: "localhost"
  port: 8080
  workers: 16
clients:
  - name: alpha
    retries: 3
    tags: [fast, stable]
  - name: beta
    retries: 5
    tags: [slow, experimental]
  - name: gamma
    retries: 1
    tags: []
features:
  - logging
  - metrics
  - "tls"
"#;

fn bench(name: &str, iterations: usize, mut f: impl FnMut()) {
    // Warm-up.
    for _ in 0..iterations / 10 {
        f();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    println!(
        "{name:<28} {iterations} iters in {elapsed:?} ({:?}/iter)",
        elapsed / iterations as u32
    );
}

fn main() {
    let iterations = 20_000usize;

    // Reference deserialization.
    let _ = serde_yaml::from_str::<Config>(YAML).expect("serde_yaml parse");
    // Validate both parsers agree.
    let ours = rmsd_yaml::from_str::<Config>(YAML).expect("rmsd_yaml parse");
    assert_eq!(ours, serde_yaml::from_str::<Config>(YAML).unwrap());

    bench("rmsd_yaml from_str", iterations, || {
        let v: Config = rmsd_yaml::from_str(YAML).unwrap();
        black_box(v);
    });
    bench("serde_yaml from_str", iterations, || {
        let v: Config = serde_yaml::from_str(YAML).unwrap();
        black_box(v);
    });

    let config = serde_yaml::from_str::<Config>(YAML).unwrap();
    bench("rmsd_yaml to_string", iterations, || {
        let s = rmsd_yaml::to_string(&config).unwrap();
        black_box(s);
    });
    bench("serde_yaml to_string", iterations, || {
        let s = serde_yaml::to_string(&config).unwrap();
        black_box(s);
    });
}
