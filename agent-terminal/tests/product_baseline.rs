use agent_terminal::chrome::{
    APPLICATION_NAME, APP_USER_MODEL_ID, UPSTREAM_UPDATE_CHECK_ENABLED, WINDOW_CLASS, WINDOW_TITLE,
};

#[test]
fn product_identity_is_distinct_and_updates_are_hard_disabled() {
    assert_eq!(APPLICATION_NAME, "Lucidity Agent Terminal");
    assert_eq!(WINDOW_TITLE, "Lucidity");
    assert_ne!(APP_USER_MODEL_ID, "org.wezfurlong.wezterm");
    assert_ne!(WINDOW_CLASS, "org.wezfurlong.wezterm");
    assert!(!UPSTREAM_UPDATE_CHECK_ENABLED);
}

#[test]
fn windows_manifest_freezes_dpi_and_utf8_policy() {
    let manifest = include_str!("../assets/windows/lucidity.manifest");
    assert!(manifest.contains("<dpiAwareness>PerMonitorV2</dpiAwareness>"));
    assert!(manifest.contains("<activeCodePage>UTF-8</activeCodePage>"));
    assert!(manifest.contains("name=\"io.lucidity.agent-terminal\""));
}

#[test]
fn weld_frame_source_matches_the_frozen_geometry() {
    let source = include_str!("../assets/windows/weld-frame.svg");
    assert!(source.contains("fill=\"#0C0E11\""));
    assert!(source.contains("fill=\"#E6EAF0\""));
    for rectangle in [
        "x=\"2\" y=\"2\" width=\"3\" height=\"12\"",
        "x=\"5\" y=\"2\" width=\"7\" height=\"3\"",
        "x=\"5\" y=\"11\" width=\"7\" height=\"3\"",
        "x=\"7\" y=\"6\" width=\"2\" height=\"4\"",
        "x=\"11\" y=\"6\" width=\"3\" height=\"4\"",
    ] {
        assert!(source.contains(rectangle), "missing rectangle {rectangle}");
    }
}

#[test]
fn ico_contains_every_required_pixel_plane() {
    let icon = include_bytes!("../assets/windows/weld-frame.ico");
    assert_eq!(u16::from_le_bytes([icon[0], icon[1]]), 0);
    assert_eq!(u16::from_le_bytes([icon[2], icon[3]]), 1);
    let count = u16::from_le_bytes([icon[4], icon[5]]) as usize;
    assert_eq!(count, 6);

    let mut sizes = Vec::new();
    for index in 0..count {
        let entry = 6 + index * 16;
        let width = if icon[entry] == 0 {
            256
        } else {
            usize::from(icon[entry])
        };
        let height = if icon[entry + 1] == 0 {
            256
        } else {
            usize::from(icon[entry + 1])
        };
        assert_eq!(width, height);
        assert_eq!(u16::from_le_bytes([icon[entry + 4], icon[entry + 5]]), 1);
        assert_eq!(u16::from_le_bytes([icon[entry + 6], icon[entry + 7]]), 32);
        sizes.push(width);
    }
    assert_eq!(sizes, [16, 24, 32, 48, 64, 256]);
}
