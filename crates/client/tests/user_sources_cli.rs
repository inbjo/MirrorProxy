use std::{fs, path::PathBuf, process::Command};

#[test]
fn short_language_commands_use_saved_origin_and_keep_unrelated_configuration() {
    let root =
        std::env::temp_dir().join(format!("mirrorproxy-user-sources-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let env = root.join("go.env");
    let cargo_home = root.join("cargo");
    let xdg = root.join("xdg");
    let appdata = root.join("appdata");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_mirrorproxy"))
            .args(args)
            .env("HOME", &root)
            .env("USERPROFILE", &root)
            .env("XDG_CONFIG_HOME", &xdg)
            .env_remove("XDG_DATA_HOME")
            .env("XDG_STATE_HOME", root.join("state"))
            .env("APPDATA", &appdata)
            .env("LOCALAPPDATA", &appdata)
            .env("GOENV", &env)
            .env("CARGO_HOME", &cargo_home)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&["use", "https://mirror.example", "--user"]);
    let paths = [
        &env,
        &cargo_home.join("config"),
        &if cfg!(windows) {
            appdata.join("pip/pip.ini")
        } else if cfg!(target_os = "macos") {
            root.join(".config/pip/pip.conf")
        } else {
            xdg.join("pip/pip.conf")
        },
    ];
    let cases = [
        ("go", "GOPRIVATE=git.example/*\r\nGOPROXY=https://old.example,direct\r\n",
         "GOPROXY=https://mirror.example/goproxy/,direct\r\n", "GOPRIVATE=git.example/*\r\n"),
        ("rust", "[net]\nretry = 7\n[source.crates-io]\nreplace-with = \"previous\"\n",
         "sparse+https://mirror.example/crates-index/", "[net]\nretry = 7"),
        ("pip", "[global]\ntimeout = 30\n[install]\nindex-url=https://old.example\n  https://old-continuation.example\n[download]\nretries = 7\n",
         "index-url = https://mirror.example/pypi/simple/", "timeout = 30"),
    ];
    for ((target, original, mirror, preserved), path) in cases.into_iter().zip(paths) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, original).unwrap();
        run(&["set", target]);
        let configured = fs::read_to_string(path).unwrap();
        assert!(configured.contains(mirror), "{target}: {configured}");
        assert!(configured.contains(preserved), "{target}: {configured}");
        assert!(!configured.contains("https://old-continuation.example"));
        if target == "pip" {
            assert_eq!(configured.matches(mirror).count(), 2);
            assert!(configured.contains("retries = 7"));
        }
        run(&["reset", target]);
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
}
