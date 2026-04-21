# G6: External-boundary edges (syscalls, file I/O, network binds) missing

## Gap

Graphify has no awareness of code that touches the OS / network / filesystem. A function that opens a socket on `:8080`, writes a sentinel file to `C:\RacingPoint\MAINTENANCE_MODE`, or reads `/proc/cpuinfo` looks identical in the graph to a pure-function counterpart. This matters for deploy analysis ("what binds port 8090?") and incident debugging ("what writes this sentinel?").

## Proposed fix

Syscall-pattern extractor with heuristic detection:

| Pattern (Rust) | Edge emitted |
|----------------|--------------|
| `TcpListener::bind(...)` / `tokio::net::TcpListener::bind` | `binds_port(N)` |
| `std::fs::write(p, ...)` / `tokio::fs::write` | `writes_file(p)` |
| `std::fs::read_to_string(p)` / `OpenOptions::new().open(p)` | `reads_file(p)` |
| `Command::new(bin)` / `tokio::process::Command::new(bin)` | `spawns_process(bin)` |
| `reqwest::Client::get(url)` / `https::request` | `http_call(url_prefix)` |

Ports + paths can be literals or string-constants; extractor resolves constant definitions when obvious, else emits `UNKNOWN` target.

## Impact

Closes ~3% of the "OS/network invisibility" gap. Enables deploy-graph queries ("who binds :8090", "who writes MAINTENANCE_MODE") without grep. Direct value for Racing Point incident debugging (P4.3 helper immediately benefits).

## References

Roadmap Tier 3 / P2.4. Pairs with G5 — both concern cross-boundary edges (G5 = cross-language; G6 = cross-process/OS).
