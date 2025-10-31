# wstunnel Manager

A cross-platform GUI application for managing multiple wstunnel instances. Built with Rust and iced-rs for a native, performant user interface.

## Features

- **Visual Management**: View all configured tunnel instances with real-time status indicators (green/running, red/stopped)
- **Easy Configuration**: Add, edit, and delete tunnel configurations through a form-based UI
- **Lifecycle Control**: Start, stop, and delete tunnels from the interface
- **Autostart Support**: Configure tunnels to start automatically when the application launches
- **Log Access**: View tunnel process logs with one click (opens in default text editor)
- **Headless Mode**: Run without GUI for server deployments and automation
- **Custom Paths**: Specify custom config file and wstunnel binary paths via CLI or config file

## Prerequisites

- Rust 1.89.0 or newer
- wstunnel binary (repository included in submodule or specify custom path)

## Quick Start

### Clone and Build

```bash
# Clone with submodules
git clone --recursive <repository-url>
cd wstunnel_manager

# Or initialize submodules if already cloned
just src get-submodules

# Build both binaries
just src build-temp
```

### Run the Application

```bash
# GUI mode
just src run

# Headless mode (for servers)
just src run-headless

# Mock mode (for development/testing)
just src run-mock
```

**Note**: All just commands must be run from the project root (not inside the wstunnel folder, as it has its own justfile).

## Configuration

Configuration is stored in `config.yaml` (created automatically on first run). Example:

```yaml
version: 1
global:
  wstunnel_binary_path: "./wstunnel.exe" # Optional: custom binary path
  log_directory: "./logs" # Optional: custom log directory
  log_retention_days: 7 # Optional: auto-delete old logs

tunnels:
  - id: "550e8400-e29b-41d4-a716-446655440000"
    tag: "SSH to production server"
    mode: Client
    cli_args: "-L socks5://127.0.0.1:1080 wss://production.example.com:443"
    autostart: true

  - id: "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
    tag: "HTTP tunnel"
    mode: Server
    cli_args: "wss://0.0.0.0:443 -r tcp://127.0.0.1:8080"
    autostart: false
```

## Usage

### GUI Mode

1. Launch the application with `just src run` or `./wstunnel_manager.exe`
2. Click "Add" to create a new tunnel configuration
3. Fill in the tunnel details:
   - Tag: A descriptive name for the tunnel
   - CLI Args: wstunnel command-line arguments
   - Autostart: Check to start automatically on launch
4. Click "Start" to launch a tunnel
5. Click "Logs" to view tunnel output
6. Click "Stop" to terminate a running tunnel
7. Click "Delete" to remove a tunnel configuration

### Headless Mode

For server deployments or automation:

```bash
# Run with default config (./config.yaml)
./wstunnel_manager.exe --headless

# Run with custom config path
./wstunnel_manager.exe --headless --config /path/to/config.yaml

# Run with custom wstunnel binary
./wstunnel_manager.exe --headless --wstunnel-path /path/to/wstunnel
```

Headless mode:

- Starts all tunnels with `autostart: true`
- Logs to configured log directory
- Gracefully shuts down all tunnels on SIGTERM/Ctrl+C
- No GUI window

### Mock Mode

For UI development without spawning real processes:

```bash
# Windows
set WSTUNNEL_MANAGER_MOCK=1 && cargo run

# Linux/macOS
WSTUNNEL_MANAGER_MOCK=1 cargo run

# Or use just (from project root)
just src run-mock
```

### Build Commands

**Note**: All commands must be run from the project root using `just src <command>`.

```bash
# Format code
just src fmt

# Check code (no compilation)
just src check

# Build debug binary
just src build

# Build release binary
just src build-release

# Run tests
just src test

# Build wstunnel from submodule (note: runs in wstunnel directory automatically)
just src build-wstunnel

# Build both binaries to ./temp folder
just src build-temp

# Clean build artifacts
just src clean
```

## Log Files

Tunnel process logs are stored in `./logs/` (or configured) with the format:

```
logs/{name}-{pid}-{timestamp}.log
```

Where `{name}` is the sanitized tunnel tag (or tunnel ID if no tag is set).

Logs contain:

- wstunnel stdout/stderr output
- Timestamps for each line
- Process exit codes and errors

Access logs by:

- Clicking "Logs" button in GUI (opens in default text editor)
- Navigating to the logs directory manually

## Future

- [ ] Make it so that headless mode has more useful commands, like a status command
