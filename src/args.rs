use std::{
    env,
    path::{Path, PathBuf},
};

use crate::exit_err;

const BUILD_DIR: &str = "build";
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
    pub project_dir: PathBuf,
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
                exit_err!("invalid target/architecture: {value}");
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
    fn ensure_unique(str: &str, from: &'static str) {
        if ARGS.contains(&str) {
            exit_err!("{from} requires an input")
        }
    }

    #[allow(clippy::ptr_arg)]
    fn validate_name(name: &String) {
        Self::ensure_unique(name, "--binary-name");
    }

    #[allow(clippy::ptr_arg)]
    fn validate_version(version: &String) {
        Self::ensure_unique(version, "--version");
        // Since Dpkg doesn't enforce semver like cargo does we will just trust the user has correctly formatted their
        // version string. See: https://manpages.ubuntu.com/manpages/xenial/man5/deb-version.5.html
    }

    fn validate_path(name: String) -> PathBuf {
        Self::ensure_unique(&name, "--project-path");
        let path = PathBuf::from(name);

        if path.is_file() {
            exit_err!("path must be a directory")
        }

        let exist_err = |path: &Path| exit_err!("{} does not exist", path.display());

        if path.is_absolute() {
            if path.exists() {
                return path;
            }
            exist_err(&path)
        }

        let mut curr_dir = env::current_dir().unwrap();
        curr_dir.push(path);

        if !curr_dir.exists() {
            exist_err(&curr_dir)
        }
        curr_dir
    }

    fn locate_valid_project_dir() -> PathBuf {
        let curr_dir = env::current_dir().unwrap();

        if curr_dir.file_name().unwrap() == BUILD_DIR {
            return curr_dir.parent().unwrap().to_owned();
        }

        curr_dir
    }

    #[inline]
    fn exit_if(is_none: bool, err: &str) {
        if is_none {
            exit_err!("{err}")
        }
    }

    pub fn parse() -> Self {
        let (mut binary_name, mut target, mut version, mut project_dir) = (None, None, None, None);
        let mut dry_run = false;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    println!(
                        "debforge v{} - Usage:\n    \
                        [-b binary-name](optional | default: will attempt to parse Cargo.toml)\n    \
                        [-v version](optional | default: will attempt to parse Cargo.toml)\n    \
                        [-t target](optional | default: x86_64-unknown-linux-gnu)\n    \
                        [-p project-path](optional | default: current directory)\n    \
                        [-d dry-run](optional | will display all found relevant deb files)",
                        env!("CARGO_PKG_VERSION")
                    );
                    std::process::exit(0);
                }
                "-b" | "--binary-name" => {
                    binary_name = args.next().inspect(Self::validate_name);
                    Self::exit_if(binary_name.is_none(), "--binary-name requires an input")
                }
                "-v" | "--version" => {
                    version = args.next().inspect(Self::validate_version);
                    Self::exit_if(version.is_none(), "--version requires an input")
                }
                "-p" | "--project-path" => {
                    project_dir = args.next().map(Self::validate_path);
                    Self::exit_if(project_dir.is_none(), "--project-path requires an input")
                }
                "-t" | "--target" => {
                    target = args.next().map(Architecture::from);
                    Self::exit_if(target.is_none(), "--target requires an input")
                }
                "-d" | "--dry-run" => dry_run = true,
                _ => {
                    exit_err!("unknown argument: {arg}");
                }
            }
        }

        Args {
            binary_name,
            version,
            project_dir: project_dir.unwrap_or_else(Self::locate_valid_project_dir),
            dry_run,
            architecture: target.unwrap_or_default(),
        }
    }
}
