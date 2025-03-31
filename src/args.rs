const ARGS: [&str; 10] = [
    "-h",
    "--help",
    "-b",
    "--binary-name",
    "-v",
    "--version",
    "-t",
    "--target",
    "-d",
    "--dry-run",
];

pub struct Args {
    pub binary_name: Option<String>,
    pub version: Option<String>,
    pub architecture: Architecture,
    pub dry_run: bool,
}

#[derive(Default, Clone, Copy)]
pub(crate) enum Architecture {
    #[default]
    Amd64,
    Arm64,
}

impl From<String> for Architecture {
    fn from(mut value: String) -> Self {
        value.make_ascii_lowercase();
        match value.as_str() {
            "x86_64-unknown-linux-gnu" | "amd" | "x86" | "x86_64" => Self::Amd64,
            "aarch64-unknown-linux-gnu" | "arm" | "aarch64" => Self::Arm64,
            _ => {
                eprintln!("Error: Invalid target/architecture: {value}");
                std::process::exit(1);
            }
        }
    }
}

impl Architecture {
    pub(crate) const fn target(self) -> &'static str {
        match self {
            Architecture::Amd64 => "x86_64-unknown-linux-gnu",
            Architecture::Arm64 => "aarch64-unknown-linux-gnu",
        }
    }

    pub(crate) const fn short(self) -> &'static str {
        match self {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
        }
    }
}

impl Args {
    fn validate_name(name: &String) {
        if ARGS.iter().any(|arg| name == arg) {
            panic!("Error: --binary-name requires an input")
        }
    }

    fn validate_version(version: &String) {
        for num_str in version.split('.') {
            if num_str.parse::<u16>().is_err() {
                panic!("Error: invalid version: '{version}'")
            }
        }
    }

    pub fn parse() -> Self {
        let (mut binary_name, mut target, mut version) = (None, None, None);
        let mut dry_run = false;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    println!(
                        "Usage:\n    \
                        [-b binary-name](optional | default: will attempt to parse Cargo.toml)\n    \
                        [-v version](optional | default: will attempt to parse Cargo.toml)\n    \
                        [-t target](optional | default: x86_64-unknown-linux-gnu)\n    \
                        [-d dry-run](optional | will display all found relevant deb files)"
                    );
                    std::process::exit(0);
                }
                "-b" | "--binary-name" => binary_name = args.next().inspect(Self::validate_name),
                "-v" | "--version" => version = args.next().inspect(Self::validate_version),
                "-t" | "--target" => target = args.next().map(Architecture::from),
                "-d" | "--dry-run" => dry_run = true,
                _ => {
                    panic!("Error: unknown argument: {arg}");
                }
            }
        }

        Args {
            binary_name,
            version,
            dry_run,
            architecture: target.unwrap_or_default(),
        }
    }
}
