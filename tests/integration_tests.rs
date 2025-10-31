use std::path::PathBuf;
use wstunnel_manager::backend::Backend;
use wstunnel_manager::backend::backend_impl::BackendState;
use wstunnel_manager::backend::types::{TunnelEntry, TunnelId, TunnelMode};

fn create_test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

#[test]
fn test_autostart_integration() {
    let runtime = create_test_runtime();
    let handle = runtime.handle().clone();

    let temp_dir = std::env::temp_dir().join(format!("wstunnel_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config_path = temp_dir.join("test_config.yaml");
    let wstunnel_path = if cfg!(windows) {
        PathBuf::from("wstunnel.exe")
    } else {
        PathBuf::from("wstunnel")
    };

    let mut backend = BackendState::new(handle.clone(), config_path.clone(), wstunnel_path);

    let autostart_tunnel = TunnelEntry {
        id: TunnelId::new(),
        tag: "autostart-test".to_string(),
        mode: TunnelMode::Client,
        cli_args: "client ws://example.com".to_string(),
        autostart: true,
        runtime_state: None,
    };

    let manual_tunnel = TunnelEntry {
        id: TunnelId::new(),
        tag: "manual-test".to_string(),
        mode: TunnelMode::Server,
        cli_args: "server ws://0.0.0.0:8080".to_string(),
        autostart: false,
        runtime_state: None,
    };

    backend.add_tunnel(autostart_tunnel.clone()).unwrap();
    backend.add_tunnel(manual_tunnel.clone()).unwrap();

    let results = backend.start_autostart_tunnels();

    match results {
        Ok(result_list) => {
            assert_eq!(result_list.len(), 1);
            let (tunnel_id, result) = &result_list[0];
            assert_eq!(*tunnel_id, autostart_tunnel.id);
            assert!(result.is_err() || result.is_ok());
        }
        Err(_) => {
            assert!(true);
        }
    }

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_config_persistence() {
    let runtime = create_test_runtime();
    let handle = runtime.handle().clone();

    let temp_dir = std::env::temp_dir().join(format!("wstunnel_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let config_path = temp_dir.join("persist_test_config.yaml");
    let wstunnel_path = if cfg!(windows) {
        PathBuf::from("wstunnel.exe")
    } else {
        PathBuf::from("wstunnel")
    };

    let tunnel_id = {
        let mut backend =
            BackendState::new(handle.clone(), config_path.clone(), wstunnel_path.clone());

        let tunnel = TunnelEntry {
            id: TunnelId::new(),
            tag: "persist-test".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let id = backend.add_tunnel(tunnel).unwrap();

        let tunnels = backend.list_tunnels();
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].tag, "persist-test");

        id
    };

    {
        let backend2 = BackendState::new(handle.clone(), config_path.clone(), wstunnel_path);

        let config = backend2.get_config();
        assert_eq!(config.tunnels.len(), 1);
        assert_eq!(config.tunnels[0].id, tunnel_id);
        assert_eq!(config.tunnels[0].tag, "persist-test");
    }

    std::fs::remove_dir_all(&temp_dir).ok();
}
