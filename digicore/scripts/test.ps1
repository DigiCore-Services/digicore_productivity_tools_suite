# Run digicore-text-expander tests with single thread for integration/ghost_follower tests
# that use global state (serial_test, integration_run_allowed, ghost_follower_tests)
cargo test -p digicore-text-expander -- --test-threads=1
