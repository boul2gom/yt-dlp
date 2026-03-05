use yt_dlp::utils::platform::{Architecture, Platform};

// ============================== Platform::detect ==============================

#[test]
fn detect_platform_returns_known_variant() {
    let platform = Platform::detect();
    let os = std::env::consts::OS;
    match os {
        "windows" => assert!(matches!(platform, Platform::Windows)),
        "linux" => assert!(matches!(platform, Platform::Linux)),
        "macos" => assert!(matches!(platform, Platform::Mac)),
        _ => assert!(matches!(platform, Platform::Unknown(_))),
    }
}

// ============================== Platform::as_str ==============================

#[test]
fn convert_platform_windows_as_str_returns_windows() {
    assert_eq!(Platform::Windows.as_str(), "windows");
}

#[test]
fn convert_platform_linux_as_str_returns_linux() {
    assert_eq!(Platform::Linux.as_str(), "linux");
}

#[test]
fn convert_platform_mac_as_str_returns_osx() {
    assert_eq!(Platform::Mac.as_str(), "osx");
}

#[test]
fn convert_platform_unknown_as_str_returns_value() {
    let p = Platform::Unknown("freebsd".to_string());
    assert_eq!(p.as_str(), "freebsd");
}

// ============================== Platform Display ==============================

#[test]
fn display_platform_windows_shows_windows_name() {
    assert_eq!(format!("{}", Platform::Windows), "Windows");
}

#[test]
fn display_platform_linux_shows_linux_name() {
    assert_eq!(format!("{}", Platform::Linux), "Linux");
}

#[test]
fn display_platform_mac_shows_mac_name() {
    assert_eq!(format!("{}", Platform::Mac), "Mac");
}

#[test]
fn display_platform_unknown_shows_os_name() {
    let p = Platform::Unknown("haiku".to_string());
    let display = format!("{}", p);
    assert!(display.contains("haiku"));
    assert!(display.contains("Unknown"));
}

// ============================== Architecture::detect ==============================

#[test]
fn detect_architecture_returns_known_variant() {
    let arch = Architecture::detect();
    let arch_str = std::env::consts::ARCH;
    match arch_str {
        "x86_64" => assert!(matches!(arch, Architecture::X64)),
        "x86" => assert!(matches!(arch, Architecture::X86)),
        "armv7l" => assert!(matches!(arch, Architecture::Armv7l)),
        "aarch64" => assert!(matches!(arch, Architecture::Aarch64)),
        _ => assert!(matches!(arch, Architecture::Unknown(_))),
    }
}

// ============================== Architecture::as_str ==============================

#[test]
fn convert_architecture_x64_as_str_returns_x64() {
    assert_eq!(Architecture::X64.as_str(), "x64");
}

#[test]
fn convert_architecture_x86_as_str_returns_x86() {
    assert_eq!(Architecture::X86.as_str(), "x86");
}

#[test]
fn convert_architecture_armv7l_as_str_returns_value() {
    assert_eq!(Architecture::Armv7l.as_str(), "armv7l");
}

#[test]
fn convert_architecture_aarch64_as_str_returns_arm64() {
    assert_eq!(Architecture::Aarch64.as_str(), "arm64");
}

#[test]
fn convert_architecture_unknown_as_str_returns_value() {
    let a = Architecture::Unknown("riscv64".to_string());
    assert_eq!(a.as_str(), "riscv64");
}

// ============================== Architecture Display ==============================

#[test]
fn display_architecture_x64_shows_x64() {
    assert_eq!(format!("{}", Architecture::X64), "x64");
}

#[test]
fn display_architecture_x86_shows_x86() {
    assert_eq!(format!("{}", Architecture::X86), "x86");
}

#[test]
fn display_architecture_armv7l_shows_capitalized() {
    assert_eq!(format!("{}", Architecture::Armv7l), "Armv7l");
}

#[test]
fn display_architecture_aarch64_shows_capitalized() {
    assert_eq!(format!("{}", Architecture::Aarch64), "Aarch64");
}

#[test]
fn display_architecture_unknown_shows_arch_name() {
    let a = Architecture::Unknown("mips".to_string());
    let display = format!("{}", a);
    assert!(display.contains("mips"));
    assert!(display.contains("Unknown"));
}
