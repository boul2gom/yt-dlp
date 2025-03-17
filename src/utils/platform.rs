//! Platform and architecture detection.

use std::fmt;

/// Represents the operating system where the program is running.
#[derive(Clone, Debug)]
pub enum Platform {
    /// The Windows operating system.
    Windows,
    /// The Linux operating system.
    Linux,
    /// The macOS operating system.
    Mac,

    /// An unknown operating system.
    Unknown(String),
}

/// Represents the architecture of the CPU where the program is running.
#[derive(Clone, Debug)]
pub enum Architecture {
    /// The x64 architecture.
    X64,
    /// The x86_64 architecture.
    X86,
    /// The ARMv7l architecture.
    Armv7l,
    /// The Aarch64 (Arm64) architecture.
    Aarch64,

    /// An unknown architecture.
    Unknown(String),
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Windows => write!(f, "Windows"),
            Platform::Linux => write!(f, "Linux"),
            Platform::Mac => write!(f, "MacOS"),
            Platform::Unknown(os) => write!(f, "Unknown: {}", os),
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X64 => write!(f, "x64"),
            Architecture::X86 => write!(f, "x86"),
            Architecture::Armv7l => write!(f, "armv7l"),
            Architecture::Aarch64 => write!(f, "aarch64"),
            Architecture::Unknown(arch) => write!(f, "Unknown: {}", arch),
        }
    }
}

impl Platform {
    /// Detects the current platform where the program is running.
    pub fn detect() -> Self {
        #[cfg(feature = "tracing")]
        tracing::debug!("Detecting current platform");

        let os = std::env::consts::OS;

        #[cfg(feature = "tracing")]
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
        #[cfg(feature = "tracing")]
        tracing::debug!("Detecting current architecture");

        let arch = std::env::consts::ARCH;

        #[cfg(feature = "tracing")]
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
