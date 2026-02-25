//! Platform and architecture detection.

/// Represents the operating system where the program is running.
#[derive(Clone, Debug, derive_more::Display)]
pub enum Platform {
    /// The Windows operating system.
    #[display("Windows")]
    Windows,
    /// The Linux operating system.
    #[display("Linux")]
    Linux,
    /// The macOS operating system.
    #[display("Mac")]
    Mac,

    /// An unknown operating system.
    #[display("Unknown(os={_0})")]
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
    #[display("Armv7l")]
    Armv7l,
    /// The Aarch64 (Arm64) architecture.
    #[display("Aarch64")]
    Aarch64,

    /// An unknown architecture.
    #[display("Unknown(arch={_0})")]
    Unknown(String),
}

impl Platform {
    /// Returns the lowercase platform identifier used in binary names.
    ///
    /// # Returns
    ///
    /// A string slice with the platform name (e.g., "windows", "linux", "osx").
    pub fn as_str(&self) -> &str {
        match self {
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Mac => "osx",
            Platform::Unknown(s) => s,
        }
    }

    /// Detects the current platform where the program is running.
    ///
    /// # Returns
    ///
    /// The detected `Platform` variant, or `Platform::Unknown` if the OS is not recognized.
    pub fn detect() -> Self {
        tracing::debug!("⚙️ Detecting current platform");

        let os = std::env::consts::OS;

        tracing::debug!(os = os, "✅ Detected platform");

        match os {
            "windows" => Platform::Windows,
            "linux" => Platform::Linux,
            "macos" => Platform::Mac,
            _ => Platform::Unknown(os.to_string()),
        }
    }
}

impl Architecture {
    /// Returns the lowercase architecture identifier used in binary names.
    ///
    /// # Returns
    ///
    /// A string slice with the architecture name (e.g., "x64", "x86", "arm64").
    pub fn as_str(&self) -> &str {
        match self {
            Architecture::X64 => "x64",
            Architecture::X86 => "x86",
            Architecture::Armv7l => "armv7l",
            Architecture::Aarch64 => "arm64",
            Architecture::Unknown(s) => s,
        }
    }

    /// Detects the current architecture of the CPU where the program is running.
    ///
    /// # Returns
    ///
    /// The detected `Architecture` variant, or `Architecture::Unknown` if the arch is not recognized.
    pub fn detect() -> Self {
        tracing::debug!("⚙️ Detecting current architecture");

        let arch = std::env::consts::ARCH;

        tracing::debug!(arch = arch, "✅ Detected architecture");

        match arch {
            "x86_64" => Architecture::X64,
            "x86" => Architecture::X86,
            "armv7l" => Architecture::Armv7l,
            "aarch64" => Architecture::Aarch64,
            _ => Architecture::Unknown(arch.to_string()),
        }
    }
}
