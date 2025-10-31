import "../common.just"

# Test commands

# Run all tests
test:
  cd {{ROOT}} && cargo test

# Run tests with mock backend
test-mock:
  set WSTUNNEL_UI_MOCK=1 && cd {{ROOT}} && cargo test
