//! Exercise the real file loader in isolated working directories.

use super::*;
use std::{env, process::Command, time::SystemTime};

#[test]
fn loads_and_validates_config_files() {
    const CASE_ENV: &str = "QUICK_SETTINGS_CONFIG_TEST_CASE";

    // Each child runs this test against its own ./quick-settings.yaml. Changing
    // the parent process's working directory would race with other UI tests.
    if let Ok(case) = env::var(CASE_ENV) {
        match case.as_str() {
            "sample" => assert_eq!(load_config().unwrap().tiles.len(), TILE_COUNT),
            "empty" => assert!(load_config().unwrap().tiles.is_empty()),
            "ordered" => assert!(matches!(
                load_config().unwrap().tiles.as_slice(),
                [TileName::Battery, TileName::Blank, TileName::Wifi]
            )),
            "overflow" => assert!(
                load_config()
                    .unwrap_err()
                    .to_string()
                    .contains("at most 8 tiles, got 9")
            ),
            "missing" | "malformed" | "unknown" | "missing_tiles" => {
                assert!(load_config().is_err());
            }
            _ => panic!("unknown config test case: {case}"),
        }
        return;
    }

    // None leaves the file absent; all other cases write the exact YAML that
    // the production loader will read. The sample also covers the eight-slot limit.
    let cases = [
        ("sample", Some(include_str!("../../quick-settings.yaml"))),
        ("empty", Some("tiles: []")),
        ("ordered", Some("tiles: [battery, blank, wifi]")),
        ("missing", None),
        ("malformed", Some("tiles: [")),
        ("unknown", Some("tiles: [unknown]")),
        ("missing_tiles", Some("{}")),
        (
            "overflow",
            Some("tiles: [blank, blank, blank, blank, blank, blank, blank, blank, blank]"),
        ),
    ];
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for (case, contents) in cases {
        let directory = env::temp_dir().join(format!(
            "quick-settings-config-{}-{unique}-{case}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        if let Some(contents) = contents {
            fs::write(directory.join("quick-settings.yaml"), contents).unwrap();
        }
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "config::tests::loads_and_validates_config_files",
                "--nocapture",
            ])
            .env(CASE_ENV, case)
            .current_dir(&directory)
            .output();

        // Clean up before reporting a child failure, so failed cases leave no files.
        fs::remove_dir_all(&directory).unwrap();
        let output = output.unwrap();
        assert!(
            output.status.success(),
            "{case}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
