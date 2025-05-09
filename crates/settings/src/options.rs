/// Options to customize parsing
#[derive(Debug, Clone)]
pub struct Options {
    /// The CLI options
    #[cfg(feature = "cli")]
    pub cli: Option<Cli>,
    /// The default config file name (loaded if no other files are specified)
    pub default_config_file: Option<&'static str>,
    /// Environment variables prefix
    ///
    /// A setting called `foo` would be read from the environment as `APP_FOO` where `APP` is the prefix.
    pub env_prefix: Option<&'static str>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            #[cfg(feature = "cli")]
            cli: None,
            default_config_file: Some("config"),
            env_prefix: Some("APP"),
        }
    }
}

/// A struct used to define how the CLI should be generated
///
/// See the [`cli!`](crate::cli) macro for a more convenient way to initialize this struct.
#[derive(Debug, Clone)]
pub struct Cli {
    /// The name of the program
    pub name: &'static str,

    /// The version of the program
    pub version: &'static str,

    /// The about of the program
    pub about: &'static str,

    /// The author of the program
    pub author: &'static str,

    /// The arguments passed to the program
    pub argv: Vec<String>,
}

/// A macro to create a CLI struct
///
/// This macro will automatically set the name, version, about, and author from
/// the cargo environment variables at compile time.
///
/// Used internally when using the [`bootstrap!`](crate::bootstrap) macro.
#[macro_export]
macro_rules! cli {
    () => {
        $crate::cli!(std::env::args().collect())
    };
    ($args:expr) => {
        $crate::Cli {
            name: option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            version: env!("CARGO_PKG_VERSION"),
            about: env!("CARGO_PKG_DESCRIPTION"),
            author: env!("CARGO_PKG_AUTHORS"),
            argv: $args,
        }
    };
}
