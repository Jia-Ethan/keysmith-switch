use std::cell::Cell;
use std::collections::HashMap;

use keysmith_switch_lib::official::{
    confirm_official_action_exec, plan_official_action_on, DetectedOfficial, OfficialAction,
    OfficialHost, OfficialProduct,
};

fn host_with(os: &str, product: OfficialProduct, detected: DetectedOfficial) -> OfficialHost {
    let mut detected_map = HashMap::new();
    detected_map.insert(product, detected);
    OfficialHost {
        os: Some(os.to_string()),
        npm_available: Some(true),
        detected: detected_map,
        dest_override: HashMap::new(),
    }
}

#[test]
fn plan_official_npm_product_is_blocked_when_npm_is_missing() {
    let mut host = host_with(
        "macos",
        OfficialProduct::Claude,
        DetectedOfficial::default(),
    );
    host.npm_available = Some(false);

    let plan = plan_official_action_on(OfficialProduct::Claude, OfficialAction::Install, &host);
    assert!(plan.argv.is_empty(), "{:?}", plan.argv);
    assert!(
        plan.blockers.iter().any(|blocker| blocker.contains("npm")),
        "{:?}",
        plan.blockers
    );

    let ran = Cell::new(false);
    let result = confirm_official_action_exec(&plan.plan_id, true, |_| {
        ran.set(true);
        Ok(())
    });
    assert!(!result.ok);
    assert!(!ran.get());
}

#[test]
fn plan_official_zcode_windows_disabled() {
    let host = host_with(
        "windows",
        OfficialProduct::Zcode,
        DetectedOfficial::default(),
    );
    let plan = plan_official_action_on(OfficialProduct::Zcode, OfficialAction::Install, &host);
    assert!(plan.argv.is_empty(), "{:?}", plan.argv);
    assert!(
        plan.blockers.iter().any(|b| b.contains("Windows")),
        "{:?}",
        plan.blockers
    );
    assert_eq!(plan.source, "https://zcode.z.ai/en/docs/install");
    assert_eq!(plan.dest, "unavailable-on-windows");
    assert!(!plan.installed);
    assert!(plan.latest_version.is_none());

    let ran = Cell::new(false);
    let result = confirm_official_action_exec(&plan.plan_id, true, |_| {
        ran.set(true);
        Ok(())
    });
    assert!(!result.ok);
    assert!(!ran.get());
    assert!(
        result.error.as_deref().unwrap_or("").contains("Windows"),
        "{:?}",
        result.error
    );
}

#[test]
fn confirm_official_action_requires_confirm() {
    let host = host_with(
        "macos",
        OfficialProduct::Claude,
        DetectedOfficial {
            executable_path: None,
            current_version: None,
            latest_version: Some("2.0.0".to_string()),
        },
    );
    let plan = plan_official_action_on(OfficialProduct::Claude, OfficialAction::Install, &host);
    assert_eq!(
        plan.argv,
        vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            "@anthropic-ai/claude-code".to_string()
        ]
    );
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert_eq!(plan.latest_version.as_deref(), Some("2.0.0"));
    assert_eq!(
        plan.source,
        "https://docs.anthropic.com/en/docs/claude-code"
    );

    let ran = Cell::new(false);
    let denied = confirm_official_action_exec(&plan.plan_id, false, |argv| {
        ran.set(true);
        let _ = argv;
        Ok(())
    });
    assert!(!denied.ok);
    assert_eq!(denied.error.as_deref(), Some("confirmation required"));
    assert!(!ran.get());

    let allowed = confirm_official_action_exec(&plan.plan_id, true, |argv| {
        ran.set(true);
        assert_eq!(argv, plan.argv.as_slice());
        Ok(())
    });
    assert!(allowed.ok, "{:?}", allowed.error);
    assert!(ran.get());
}

#[test]
fn plan_official_grok_latest_is_null() {
    let host = host_with(
        "macos",
        OfficialProduct::Grok,
        DetectedOfficial {
            executable_path: Some("/Users/ethan/.local/bin/grok".to_string()),
            current_version: Some("1.0.4".to_string()),
            latest_version: Some("must-not-leak".to_string()),
        },
    );
    let plan = plan_official_action_on(OfficialProduct::Grok, OfficialAction::Update, &host);
    assert!(plan.installed);
    assert_eq!(plan.current_version.as_deref(), Some("1.0.4"));
    assert!(plan.latest_version.is_none());
    assert!(plan.argv.is_empty());
    assert!(plan.blockers.iter().any(|b| b.contains("latest feed")));
}

#[test]
fn plan_official_codex_update_uses_npm_argv() {
    let host = host_with(
        "macos",
        OfficialProduct::Codex,
        DetectedOfficial {
            executable_path: Some("/usr/local/bin/codex".to_string()),
            current_version: Some("0.1.0".to_string()),
            latest_version: Some("0.2.0".to_string()),
        },
    );
    let plan = plan_official_action_on(OfficialProduct::Codex, OfficialAction::Update, &host);
    assert_eq!(
        plan.argv,
        vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            "@openai/codex@latest".to_string()
        ]
    );
    assert_eq!(plan.source, "https://www.npmjs.com/package/@openai/codex");
}

#[test]
fn plan_official_zcode_macos_manual_install() {
    let host = host_with(
        "macos",
        OfficialProduct::Zcode,
        DetectedOfficial {
            executable_path: Some("/Applications/ZCode.app/Contents/MacOS/ZCode".to_string()),
            current_version: Some("3.7.7".to_string()),
            latest_version: Some("ignored".to_string()),
        },
    );
    let plan = plan_official_action_on(OfficialProduct::Zcode, OfficialAction::Update, &host);
    assert!(plan.installed);
    assert!(plan.latest_version.is_none());
    assert_eq!(plan.dest, "/Applications/ZCode.app");
    assert!(plan.argv.is_empty());
    assert!(!plan.blockers.is_empty());
}
