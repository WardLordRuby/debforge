mod deb_files;

use std::{
    collections::HashMap,
    env,
    fs::{self, DirEntry},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use crate::args::*;
use deb_files::*;

const PROJECT_ENV_VAR: &str = "PROJ_DIR";
const TEMP_DIR: &str = "tmp";
const BUILD_DIR: &str = "build";
const SEARCH_DIRS: [SearchDir; 3] = [SearchDir::Assets, SearchDir::Build, SearchDir::Debian];
const REQUIRED_DEB_FILES: [FileType; 3] =
    [FileType::Control, FileType::Changelog, FileType::Copyright];

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

macro_rules! exit_err {
    ($($arg:tt)*) => {{
        eprint!("{PKG_NAME}: Error ");
        eprintln!($($arg)*);
        std::process::exit(1);
    }};
}

type DebFiles = HashMap<FileType, PathBuf>;

pub struct Forge {
    vars: Variables,
    files: DebFiles,
}

struct Variables {
    project_dir: PathBuf,
    binary_name: String,
    linux_binary_name: String,
    version: String,
    architecture: Architecture,
}

impl Variables {
    fn replacements(&self) -> [(&'static str, &str); 5] {
        [
            ("$BinaryName", &self.binary_name),
            ("$LinuxBinaryName", &self.linux_binary_name),
            ("$Version", &self.version),
            ("$Target", self.architecture.target()),
            ("$Architecture", self.architecture.short()),
        ]
    }

    fn write_file(&self, file_type: FileType, input: &Path) -> io::Result<()> {
        let mut output_dir = self.get_file_type_path(file_type);
        fs::create_dir_all(&output_dir)?;
        output_dir.push(
            file_type
                .output_file_name(&self.linux_binary_name)
                .as_path(),
        );

        if file_type.is_icon() {
            fs::copy(input, output_dir)?;
            return Ok(());
        }

        let input = fs::File::open(input)?;
        let input = BufReader::new(input);

        let output = fs::File::create(&output_dir)?;
        let mut output = BufWriter::new(output);

        let replacements = self.replacements();

        for line in input.lines() {
            let mut line = line?;
            for (key, value) in replacements {
                line = line.replace(key, value);
            }

            line.push('\n');
            output.write_all(line.as_bytes())?;
        }

        Ok(())
    }
}

trait DebCollector {
    fn try_insert_deb(&mut self, entry: &DirEntry, dry_run: bool);
}

impl DebCollector for DebFiles {
    fn try_insert_deb(&mut self, entry: &DirEntry, dry_run: bool) {
        if let Some(deb_file) = entry.debian_file() {
            if self.insert(deb_file, entry.path()).is_some() {
                exit_err!("Error: found more than 1 {deb_file:?} file")
            }
            if dry_run {
                println!("Found {deb_file:?} file")
            }
        }
    }
}

fn locate_valid_project_dir() -> PathBuf {
    let curr_dir = env::var(PROJECT_ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_dir().unwrap());

    if !curr_dir.is_dir() {
        exit_err!(
            "Error: {} is not a valid project directory",
            curr_dir.display()
        )
    }

    if curr_dir.file_name().unwrap() == BUILD_DIR {
        return curr_dir.parent().unwrap().to_owned();
    }

    curr_dir
}

#[derive(Clone, Copy)]
enum SearchDir {
    Assets,
    Build,
    Debian,
}

impl SearchDir {
    const fn name(self) -> &'static str {
        match self {
            SearchDir::Assets => "assets",
            SearchDir::Build => "build",
            SearchDir::Debian => "debian",
        }
    }

    fn scan(self, deb_files: &mut DebFiles, path: PathBuf, dry_run: bool) -> io::Result<()> {
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            match self {
                SearchDir::Assets | SearchDir::Debian if file_type.is_dir() => {
                    self.scan(deb_files, entry.path(), dry_run)?
                }
                SearchDir::Build if file_type.is_dir() => {
                    let file_name = entry.file_name();
                    if !dry_run && file_name == TEMP_DIR {
                        fs::remove_dir_all(entry.path())?;
                        println!("Reset contents of ~/build/tmp")
                    } else if file_name == SearchDir::Debian.name() {
                        SearchDir::Debian.scan(deb_files, entry.path(), dry_run)?
                    }
                }
                _ if file_type.is_file() => deb_files.try_insert_deb(&entry, dry_run),
                _ => (),
            }
        }
        Ok(())
    }
}

impl Forge {
    fn parse_toml(
        toml: &Path,
        binary_name: &mut Option<String>,
        version: &mut Option<String>,
    ) -> io::Result<()> {
        let toml = fs::File::open(toml)?;
        let reader = BufReader::new(toml);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if binary_name.is_none() {
                if let Some(name) = line
                    .strip_prefix("name = \"")
                    .and_then(|rest| rest.strip_suffix('\"'))
                {
                    *binary_name = Some(String::from(name))
                }
            } else if version.is_none() {
                if let Some(version_str) = line
                    .strip_prefix("version = \"")
                    .and_then(|rest| rest.strip_suffix('\"'))
                {
                    *version = Some(String::from(version_str))
                }
            }

            if binary_name.is_some() && version.is_some() {
                break;
            }
        }
        Ok(())
    }

    pub fn from(mut args: Args) -> io::Result<Self> {
        let project_dir = locate_valid_project_dir();
        let mut deb_files = HashMap::new();
        let mut toml_found = false;

        for entry in fs::read_dir(&project_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let file_name = entry.file_name();

            if file_type.is_dir() {
                if let Some(search_dir) = SEARCH_DIRS.iter().find(|valid| file_name == valid.name())
                {
                    search_dir.scan(&mut deb_files, entry.path(), args.dry_run)?;
                }
            } else if file_type.is_file() {
                if file_name == "Cargo.toml" {
                    toml_found = true;
                    if args.binary_name.is_none() || args.version.is_none() {
                        Self::parse_toml(&entry.path(), &mut args.binary_name, &mut args.version)?
                    }
                    continue;
                }

                deb_files.try_insert_deb(&entry, args.dry_run)
            }
        }

        if args.binary_name.is_none() || args.version.is_none() {
            if toml_found {
                exit_err!("Failed to parse Cargo.toml")
            } else {
                exit_err!("Could not find Cargo.toml")
            }
        }

        let binary_name = args.binary_name.unwrap();

        for required in REQUIRED_DEB_FILES.iter() {
            if !deb_files.contains_key(required) {
                exit_err!("Could not locate a {required:?} file")
            }
        }

        let project = Self {
            vars: Variables {
                project_dir,
                linux_binary_name: binary_name.replace('_', "-"),
                binary_name,
                version: args.version.unwrap(),
                architecture: args.architecture,
            },
            files: deb_files,
        };

        if args.dry_run {
            println!("{PKG_NAME}: Success valid project file structure");
            std::process::exit(0)
        }

        Ok(project)
    }

    pub fn forge(self) -> io::Result<()> {
        for (&file, path) in self.files.iter() {
            self.vars.write_file(file, path)?
        }

        println!(
            "{PKG_NAME}: Successfully imported {} files",
            self.files.len()
        );
        Ok(())
    }
}
