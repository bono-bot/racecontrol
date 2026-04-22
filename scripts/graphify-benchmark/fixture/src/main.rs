// P5.4 benchmark fixture.
// Hand-authored "known answer" Rust code. The reference-graph.json alongside
// this file enumerates every edge a perfect extractor would emit. The scorer
// compares graphify's actual output against that reference.
//
// DO NOT edit without updating reference-graph.json in the same commit —
// scorer results become meaningless otherwise.

use std::fs;
use std::process::Command;

// -------- Category A: function calls (graphify SHOULD catch these) --------

fn a_calls_b() {
    b_is_called();
}

fn b_is_called() {
    c_leaf();
}

fn c_leaf() {
    // terminal
}

// -------- Category B: struct-field reads/writes (G3 gap — MISSED) --------

struct Config {
    name: String,
    count: u32,
    enabled: bool,
}

fn reads_config_fields(cfg: &Config) -> bool {
    // Should emit reads_field edges to Config.name, Config.count, Config.enabled
    let _ = cfg.name.clone();
    let _ = cfg.count + 1;
    cfg.enabled
}

fn writes_config_field(cfg: &mut Config) {
    // Should emit writes_field edge to Config.enabled
    cfg.enabled = false;
}

// -------- Category C: external boundary (G6 gap — MISSED) --------

fn spawns_subprocess() {
    // Should emit spawns_process(node) edge
    let _ = Command::new("node").arg("--version").spawn();
}

fn reads_file() -> String {
    // Should emit reads_file("/etc/hostname") edge
    fs::read_to_string("/etc/hostname").unwrap_or_default()
}

// -------- Category D: dynamic dispatch (G16 + runtime gap — MISSED) --------

trait Handler {
    fn handle(&self) -> u32;
}

struct FastHandler;
impl Handler for FastHandler {
    fn handle(&self) -> u32 { 1 }
}

struct SlowHandler;
impl Handler for SlowHandler {
    fn handle(&self) -> u32 { 42 }
}

fn dispatch(h: &dyn Handler) -> u32 {
    // Pure AST cannot know whether h.handle() → FastHandler or SlowHandler.
    // Runtime-trace extractor (P5.1) would emit dispatched_to edges.
    h.handle()
}

// -------- Category E: macro-generated code (G21 gap — ambiguous) --------

fn uses_println() {
    // graphify typically emits println! as a hub-style label
    println!("hello from fixture");
}

fn main() {
    a_calls_b();
    let mut cfg = Config { name: "test".into(), count: 3, enabled: true };
    let _ = reads_config_fields(&cfg);
    writes_config_field(&mut cfg);
    spawns_subprocess();
    let _ = reads_file();
    let h: Box<dyn Handler> = Box::new(FastHandler);
    let _ = dispatch(&*h);
    uses_println();
}
