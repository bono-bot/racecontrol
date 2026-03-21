// Library target for unit testing modules that don't require ONNX Runtime linking.
// The full binary is built via main.rs.

// Only expose modules needed for testable code.
// Detection types are needed by recognition quality gates.
pub mod detection {
    pub mod types;
}
pub mod recognition;
