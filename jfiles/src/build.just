import "../common.just"

# Build commands

fmt:
  cd {{ROOT}} && cargo fmt --all

# Check all targets
check:
  cd {{ROOT}} && cargo check --all-targets

# Build debug binary
build:
  cd {{ROOT}} && cargo build

# Build release binary
build-release:
  cd {{ROOT}} && cargo build --release

# Build wstunnel (from submodule)
build-wstunnel:
  cd {{ROOT}}/wstunnel && cargo build --release

# Build both binaries to temp folder
build-temp: build-release build-wstunnel
  mkdir -p {{ROOT}}/temp
  cp {{ROOT}}/target/release/wstunnel_ui.exe {{ROOT}}/temp/ || cp {{ROOT}}/target/release/wstunnel_ui {{ROOT}}/temp/
  cp {{ROOT}}/wstunnel/target/release/wstunnel.exe {{ROOT}}/temp/ || cp {{ROOT}}/wstunnel/target/release/wstunnel {{ROOT}}/temp/

# Run the UI in GUI mode
run:
  cd {{ROOT}} && cargo run

# Run in headless mode
run-headless:
  cd {{ROOT}} && cargo run -- --headless

# Run with mock backend (Windows)
run-mock:
  set WSTUNNEL_UI_MOCK=1 && cd {{ROOT}} && cargo run

# Run from temp directory (portable package)
run-temp: build-temp
  cd {{ROOT}}/temp && ./wstunnel_ui.exe || ./wstunnel_ui

# Clean build artifacts, logs, and temp files
clean:
  cd {{ROOT}} && cargo clean
  rm -rf {{ROOT}}/logs || true
  rm -rf {{ROOT}}/temp || true
  rm -f {{ROOT}}/wstunnel_config.yaml || true

fix:
  cd {{ROOT}} && cargo clippy --workspace --allow-dirty --fix