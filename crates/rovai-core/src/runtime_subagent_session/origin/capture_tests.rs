use super::*;

// Pure launch-snapshot owner: no filesystem, SQLite, process or ambient env.
// The existing Context owner separately owns binding persistence and rollback.
#[test]
fn final_launch_roots_keep_adapter_precedence_and_qoder_node_path_semantics() {
    let cwd = if cfg!(windows) {
        PathBuf::from(r"Q:\work\app")
    } else {
        PathBuf::from("/work/app")
    };
    let home = if cfg!(windows) {
        PathBuf::from(r"Q:\users\captured")
    } else {
        PathBuf::from("/users/captured")
    };
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let env = |values: &[(&str, &str)]| -> BTreeMap<OsString, OsString> {
        let mut map = BTreeMap::from([(OsString::from(key), home.clone().into_os_string())]);
        for (name, value) in values {
            map.insert((*name).into(), (*value).into());
        }
        map
    };
    let root = |kind, values: &[(&str, &str)]| {
        CapturedNativeSource::capture(kind, &env(values), &cwd, "captured-host")
            .unwrap()
            .home
    };
    for (kind, variable, default) in [
        (AdapterKind::CodexCli, "CODEX_HOME", ".codex"),
        (AdapterKind::GrokBuild, "GROK_HOME", ".grok"),
        (AdapterKind::ClaudeCodeCli, "CLAUDE_CONFIG_DIR", ".claude"),
        (AdapterKind::QoderCli, "QODER_CONFIG_DIR", ".qoder"),
    ] {
        assert_eq!(root(kind, &[]), home.join(default));
        assert_eq!(root(kind, &[(variable, "")]), home.join(default));
        assert_eq!(root(kind, &[(variable, "独立来源")]), cwd.join("独立来源"));
        let mut map = env(&[
            ("CODEX_HOME", "codex"),
            ("GROK_HOME", "grok"),
            ("CLAUDE_CONFIG_DIR", "claude"),
            ("QODER_CONFIG_DIR", "qoder"),
            ("API_TOKEN", "SECRET_DO_NOT_CAPTURE"),
        ]);
        let original = CapturedNativeSource::capture(kind, &map, &cwd, "original-host").unwrap();
        map.insert(variable.into(), "later".into());
        assert_eq!(
            original.home,
            cwd.join(match kind {
                AdapterKind::CodexCli => "codex",
                AdapterKind::GrokBuild => "grok",
                AdapterKind::ClaudeCodeCli => "claude",
                AdapterKind::QoderCli => "qoder",
                _ => unreachable!(),
            })
        );
        assert_eq!(original.adapter_kind(), kind);
        assert_eq!(original.working_directory, cwd);
        assert!(!format!("{original:?}").contains(original.home.to_str().unwrap()));
        assert!(!format!("{original:?}").contains("SECRET_DO_NOT_CAPTURE"));
        assert!(CapturedNativeSource::capture(kind, &BTreeMap::new(), &cwd, "host").is_none());
        assert!(CapturedNativeSource::capture(kind, &map, Path::new("relative"), "host").is_none());
        assert!(CapturedNativeSource::capture(kind, &map, &cwd, "../invalid-host").is_none());
        assert!(
            CapturedNativeSource::capture(kind, &env(&[(variable, "bad\0root")]), &cwd, "host")
                .is_none()
        );
    }
    let qoder = AdapterKind::QoderCli;
    assert!(
        CapturedNativeSource::capture(AdapterKind::GeminiCli, &env(&[]), &cwd, "host").is_none()
    );
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../integration/verification/native-launch-origin/qoder-root-vectors.json"
    ))
    .unwrap();
    let mut compared = 0;
    for case in reference["results"].as_array().unwrap() {
        if case["platform"] != if cfg!(windows) { "win32" } else { "posix" } {
            continue;
        }
        let mut environment = BTreeMap::from([(
            OsString::from(key),
            OsString::from(case["home"].as_str().unwrap()),
        )]);
        for (name, value) in case["environment"].as_object().unwrap() {
            environment.insert(name.into(), value.as_str().unwrap().into());
        }
        let captured = CapturedNativeSource::capture(
            qoder,
            &environment,
            Path::new(case["cwd"].as_str().unwrap()),
            "host",
        );
        if case["expectedError"] == true {
            assert!(captured.is_none(), "publisher rejects {case}");
        } else {
            assert_eq!(
                captured.unwrap().home,
                Path::new(case["expectedHome"].as_str().unwrap()),
                "publisher source: {case}"
            );
        }
        compared += 1;
    }
    assert_eq!(
        compared, 66,
        "both fixed publisher versions are covered on this platform"
    );
    #[cfg(windows)]
    {
        let mut environment = env(&[("QODER_CONFIG_DIR", "Z:relative")]);
        assert_eq!(
            CapturedNativeSource::capture(qoder, &environment, &cwd, "host")
                .unwrap()
                .home,
            Path::new(r"Z:\relative")
        );
        environment.insert("=Z:".into(), r"Z:\saved".into());
        assert_eq!(
            CapturedNativeSource::capture(qoder, &environment, &cwd, "host")
                .unwrap()
                .home,
            Path::new(r"Z:\saved\relative")
        );
        environment.insert("=Z:".into(), r"Q:\wrong-drive".into());
        assert_eq!(
            CapturedNativeSource::capture(qoder, &environment, &cwd, "host")
                .unwrap()
                .home,
            Path::new(r"Z:\relative")
        );
    }
    #[cfg(windows)]
    assert_eq!(
        root(
            AdapterKind::ClaudeCodeCli,
            &[("claude_config_dir", "case-insensitive")]
        ),
        cwd.join("case-insensitive")
    );
}
