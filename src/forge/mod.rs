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

pub struct Forge {
    vars: Variables,
    files: PackageFiles,
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

    fn write_file(&self, input: &Path, file_type: FileType) -> io::Result<()> {
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

type DebFiles = HashMap<FileType, PathBuf>;

struct PackageFiles {
    required: [(FileType, PathBuf); 3],
    optional: DebFiles,
}

impl PackageFiles {
    fn len(&self) -> usize {
        self.required.len() + self.optional.len()
    }
}

trait DebCollector {
    fn try_insert_deb(&mut self, entry: &DirEntry, dry_run: bool);
}

impl DebCollector for DebFiles {
    fn try_insert_deb(&mut self, entry: &DirEntry, dry_run: bool) {
        if let Some(deb_file) = entry.debian_file() {
            if self.insert(deb_file, entry.path()).is_some() {
                panic!("Error: found more than 1 {deb_file:?} file")
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
        panic!(
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
                panic!("Error: Failed to parse Cargo.toml")
            } else {
                panic!("Error: Could not find Cargo.toml")
            }
        }

        let binary_name = args.binary_name.unwrap();
        let required = [
            deb_files
                .remove_entry(&FileType::Control)
                .expect("Error: Could not locate a control file"),
            deb_files
                .remove_entry(&FileType::Changelog)
                .expect("Error: Could not locate a changelog file"),
            deb_files
                .remove_entry(&FileType::Copyright)
                .expect("Error: Could not find copyright file"),
        ];

        let project = Self {
            vars: Variables {
                project_dir,
                linux_binary_name: binary_name.replace('_', "-"),
                binary_name,
                version: args.version.unwrap(),
                architecture: args.architecture,
            },
            files: PackageFiles {
                required,
                optional: deb_files,
            },
        };

        if args.dry_run {
            println!("Valid project file structure");
            std::process::exit(0)
        }

        Ok(project)
    }

    pub fn forge(self) -> io::Result<()> {
        let total = self.files.len();
        for (file, path) in self.files.required.into_iter().chain(self.files.optional) {
            self.vars.write_file(&path, file)?
        }

        println!("Successfully imported {total} files");
        Ok(())
    }
}
