//! Platform and architecture detection.

/// Represents the operating system where the program is running.
#[derive(Clone, Debug, derive_more::Display)]
pub enum Platform {
    /// The Windows operating system.
    #[display("windows")]
    Windows,
    /// The Linux operating system.
    #[display("linux")]
    Linux,
    /// The macOS operating system.
    #[display("osx")]
    Mac,

    /// An unknown operating system.
    #[display("Unknown: {_0}")]
    Unknown(String),
}

/// Represents the architecture of the CPU where the program is running.
#[derive(Clone, Debug, derive_more::Display)]
pub enum Architecture {
    /// The x64 architecture.
    #[display("x64")]
    X64,
    /// The x86_64 architecture.
    #[display("x86")]
    X86,
    /// The ARMv7l architecture.
    #[display("armv7l")]
    Armv7l,
    /// The Aarch64 (Arm64) architecture.
    #[display("arm64")]
    Aarch64,

    /// An unknown architecture.
    #[display("Unknown: {_0}")]
    Unknown(String),
}

impl Platform {
    /// Detects the current platform where the program is running.
    pub fn detect() -> Self {
        tracing::debug!("Detecting current platform");

        let os = std::env::consts::OS;

        tracing::debug!("Detected platform: {}", os);

        match os {
            "windows" => Platform::Windows,
            "linux" => Platform::Linux,
            "macos" => Platform::Mac,
            _ => Platform::Unknown(os.to_string()),
        }
    }
}

impl Architecture {
    /// Detects the current architecture of the CPU where the program is running.
    pub fn detect() -> Self {
        tracing::debug!("Detecting current architecture");

        let arch = std::env::consts::ARCH;

        tracing::debug!("Detected architecture: {}", arch);

        match arch {
            "x86_64" => Architecture::X64,
            "x86" => Architecture::X86,
            "armv7l" => Architecture::Armv7l,
            "aarch64" => Architecture::Aarch64,
            _ => Architecture::Unknown(arch.to_string()),
        }
    }
}
