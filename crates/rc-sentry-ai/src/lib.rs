// Library target for unit testing modules that don't require ONNX Runtime linking.
// The full binary is built via main.rs.
//
// Only expose modules needed for testable code.
// Modules that depend on ort (arcface) are excluded to avoid linker issues.

pub mod detection {
    pub mod types;
}

pub mod recognition {
    pub mod alignment;
    pub mod clahe;
    pub mod db;
    pub mod gallery;
    pub mod quality;
    pub mod tracker;
    pub mod types;
}

pub mod enrollment {
    pub mod types;
}

pub mod attendance {
    pub mod db;
    // engine depends on config (binary-only module), excluded from lib target
}
