//! Main entry point for all integration tests

// Include all integration test modules
mod integration {
    mod test_tui_flows;
}

// Re-export tests from modules so they can be run
pub use integration::*;
