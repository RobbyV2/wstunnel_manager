use std::path::PathBuf;
use wstunnel_manager::backend::Backend;
use wstunnel_manager::backend::backend_impl::BackendState;
use wstunnel_manager::backend::types::{Config, GlobalSettings, TunnelEntry, TunnelId, TunnelMode};

mod config_validation {
    use super::*;

    #[test]
    fn valid_config() {
        let config = Config {
            version: 1,
            global: GlobalSettings::default(),
            tunnels: vec![TunnelEntry {
                id: TunnelId::new(),
                tag: "test-tunnel".to_string(),
                mode: TunnelMode::Client,
                cli_args: "client ws://example.com".to_string(),
                autostart: false,
                runtime_state: None,
            }],
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn duplicate_tunnel_ids() {
        let id = TunnelId::new();
        let config = Config {
            version: 1,
            global: GlobalSettings::default(),
            tunnels: vec![
                TunnelEntry {
                    id,
                    tag: "tunnel-1".to_string(),
                    mode: TunnelMode::Client,
                    cli_args: "client ws://example.com".to_string(),
                    autostart: false,
                    runtime_state: None,
                },
                TunnelEntry {
                    id,
                    tag: "tunnel-2".to_string(),
                    mode: TunnelMode::Server,
                    cli_args: "server ws://0.0.0.0:8080".to_string(),
                    autostart: false,
                    runtime_state: None,
                },
            ],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Duplicate tunnel ID")
        );
    }

    #[test]
    fn invalid_config_version() {
        let config = Config {
            version: 999,
            global: GlobalSettings::default(),
            tunnels: vec![],
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported config version")
        );
    }
}

mod tunnel_entry_validation {
    use super::*;

    #[test]
    fn valid_tunnel_entry() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "valid-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: true,
            runtime_state: None,
        };

        assert!(entry.validate().is_ok());
    }

    #[test]
    fn empty_or_whitespace_tag() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "   ".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn tag_too_long() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "a".repeat(101),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tag too long"));
    }

    #[test]
    fn empty_cli_args() {
        let entry = TunnelEntry {
            id: TunnelId::new(),
            tag: "test-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "   ".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let result = entry.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("CLI arguments cannot be empty")
        );
    }

    #[test]
    fn autostart_flag_behavior() {
        let entry_with_autostart = TunnelEntry {
            id: TunnelId::new(),
            tag: "autostart-tunnel".to_string(),
            mode: TunnelMode::Server,
            cli_args: "server ws://0.0.0.0:8080".to_string(),
            autostart: true,
            runtime_state: None,
        };

        assert!(entry_with_autostart.validate().is_ok());
        assert!(entry_with_autostart.autostart);

        let entry_without_autostart = TunnelEntry {
            id: TunnelId::new(),
            tag: "manual-tunnel".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        assert!(entry_without_autostart.validate().is_ok());
        assert!(!entry_without_autostart.autostart);
    }
}

mod log_retention {
    use super::*;

    #[test]
    fn defaults_to_none() {
        let settings = GlobalSettings::default();
        assert!(settings.log_retention_days.is_none());
    }

    #[test]
    fn validates_minimum_value() {
        let settings = GlobalSettings {
            wstunnel_binary_path: None,
            log_directory: PathBuf::from("./logs"),
            log_retention_days: Some(0),
        };

        let result = settings.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must be between 1 and 3650")
        );
    }

    #[test]
    fn validates_maximum_value() {
        let settings = GlobalSettings {
            wstunnel_binary_path: None,
            log_directory: PathBuf::from("./logs"),
            log_retention_days: Some(3651),
        };

        let result = settings.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must be between 1 and 3650")
        );
    }

    #[test]
    fn accepts_valid_values() {
        let test_cases = vec![
            (Some(1), true),
            (Some(7), true),
            (Some(30), true),
            (Some(365), true),
            (Some(3650), true),
            (None, true),
        ];

        for (retention_days, should_pass) in test_cases {
            let settings = GlobalSettings {
                wstunnel_binary_path: None,
                log_directory: PathBuf::from("./logs"),
                log_retention_days: retention_days,
            };

            let result = settings.validate();
            assert_eq!(
                result.is_ok(),
                should_pass,
                "Expected retention_days {:?} to {}",
                retention_days,
                if should_pass { "pass" } else { "fail" }
            );
        }
    }
}

mod cli_args_parsing {
    use clap::Parser;
    use std::path::PathBuf;

    #[derive(Parser, Debug)]
    #[command(name = "wstunnel_manager")]
    struct Args {
        #[arg(long)]
        headless: bool,

        #[arg(long)]
        config: Option<PathBuf>,

        #[arg(long)]
        wstunnel_path: Option<PathBuf>,
    }

    #[test]
    fn headless_flag() {
        let args = Args::parse_from(["wstunnel_manager", "--headless"]);
        assert!(args.headless);
        assert!(args.config.is_none());
        assert!(args.wstunnel_path.is_none());
    }

    #[test]
    fn config_path_flag() {
        let args = Args::parse_from(["wstunnel_manager", "--config", "custom_config.yaml"]);
        assert!(!args.headless);
        assert_eq!(args.config.unwrap(), PathBuf::from("custom_config.yaml"));
    }

    #[test]
    fn wstunnel_path_flag() {
        let args = Args::parse_from(["wstunnel_manager", "--wstunnel-path", "/usr/bin/wstunnel"]);
        assert!(!args.headless);
        assert_eq!(
            args.wstunnel_path.unwrap(),
            PathBuf::from("/usr/bin/wstunnel")
        );
    }

    #[test]
    fn all_flags_combined() {
        let args = Args::parse_from([
            "wstunnel_manager",
            "--headless",
            "--config",
            "test.yaml",
            "--wstunnel-path",
            "./wstunnel",
        ]);
        assert!(args.headless);
        assert_eq!(args.config.unwrap(), PathBuf::from("test.yaml"));
        assert_eq!(args.wstunnel_path.unwrap(), PathBuf::from("./wstunnel"));
    }
}

mod backend_integration {
    use super::*;

    fn create_test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Runtime::new().unwrap()
    }

    fn create_temp_test_dir() -> PathBuf {
        let temp_dir = std::env::temp_dir().join(format!("wstunnel_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        temp_dir
    }

    fn get_wstunnel_path() -> PathBuf {
        match cfg!(windows) {
            true => PathBuf::from("wstunnel.exe"),
            false => PathBuf::from("wstunnel"),
        }
    }

    #[test]
    fn autostart_tunnels() {
        let runtime = create_test_runtime();
        let handle = runtime.handle().clone();
        let temp_dir = create_temp_test_dir();

        let config_path = temp_dir.join("test_config.yaml");
        let wstunnel_path = get_wstunnel_path();

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
        if let Ok(result_list) = results {
            assert_eq!(result_list.len(), 1);
            let (tunnel_id, _result) = &result_list[0];
            assert_eq!(*tunnel_id, autostart_tunnel.id);
        }

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn config_persistence() {
        let runtime = create_test_runtime();
        let handle = runtime.handle().clone();
        let temp_dir = create_temp_test_dir();

        let config_path = temp_dir.join("persist_test_config.yaml");
        let wstunnel_path = get_wstunnel_path();

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

    #[test]
    fn add_and_list_tunnels() {
        let runtime = create_test_runtime();
        let handle = runtime.handle().clone();
        let temp_dir = create_temp_test_dir();

        let config_path = temp_dir.join("add_list_test.yaml");
        let wstunnel_path = get_wstunnel_path();

        let mut backend = BackendState::new(handle, config_path, wstunnel_path);

        assert_eq!(backend.list_tunnels().len(), 0);

        let tunnel1 = TunnelEntry {
            id: TunnelId::new(),
            tag: "tunnel-1".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://server1.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let tunnel2 = TunnelEntry {
            id: TunnelId::new(),
            tag: "tunnel-2".to_string(),
            mode: TunnelMode::Server,
            cli_args: "server ws://0.0.0.0:8080".to_string(),
            autostart: true,
            runtime_state: None,
        };

        backend.add_tunnel(tunnel1.clone()).unwrap();
        backend.add_tunnel(tunnel2.clone()).unwrap();

        let tunnels = backend.list_tunnels();
        assert_eq!(tunnels.len(), 2);
        assert!(tunnels.iter().any(|t| t.tag == "tunnel-1"));
        assert!(tunnels.iter().any(|t| t.tag == "tunnel-2"));

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn delete_tunnel() {
        let runtime = create_test_runtime();
        let handle = runtime.handle().clone();
        let temp_dir = create_temp_test_dir();

        let config_path = temp_dir.join("delete_test.yaml");
        let wstunnel_path = get_wstunnel_path();

        let mut backend = BackendState::new(handle, config_path, wstunnel_path);

        let tunnel = TunnelEntry {
            id: TunnelId::new(),
            tag: "to-delete".to_string(),
            mode: TunnelMode::Client,
            cli_args: "client ws://example.com".to_string(),
            autostart: false,
            runtime_state: None,
        };

        let id = backend.add_tunnel(tunnel).unwrap();
        assert_eq!(backend.list_tunnels().len(), 1);

        backend.delete_tunnel(id).unwrap();
        assert_eq!(backend.list_tunnels().len(), 0);

        std::fs::remove_dir_all(&temp_dir).ok();
    }
}

mod global_settings {
    use super::*;

    #[test]
    fn default_values() {
        let settings = GlobalSettings::default();
        assert!(settings.wstunnel_binary_path.is_none());
        assert_eq!(settings.log_directory, PathBuf::from(".").join("logs"));
        assert!(settings.log_retention_days.is_none());
    }

    #[test]
    fn custom_log_directory() {
        let settings = GlobalSettings {
            wstunnel_binary_path: None,
            log_directory: PathBuf::from("/var/log/wstunnel"),
            log_retention_days: None,
        };

        assert!(settings.validate().is_ok());
        assert_eq!(settings.log_directory, PathBuf::from("/var/log/wstunnel"));
    }
}
