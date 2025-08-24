//! Main entry point for all unit tests

// Include all unit test modules
mod unit {
    mod test_init_and_create;
    mod test_list_and_search;
    mod test_tui_components;
}

// Re-export tests from modules so they can be run
pub use unit::*;
