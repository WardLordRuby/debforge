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

pub(crate) const PKG_NAME: &str = env!("CARGO_PKG_NAME");

const TEMP_DIR: &str = "tmp";
const SEARCH_DIRS: [SearchDir; 3] = [SearchDir::Assets, SearchDir::Build, SearchDir::Debian];
const REQUIRED_DEB_FILES: [FileType; 3] =
    [FileType::Control, FileType::Changelog, FileType::Copyright];

#[macro_export]
macro_rules! exit_err {
    ($($arg:tt)*) => {{
        eprint!("{}: Error ", $crate::forge::PKG_NAME);
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

impl Args {
    #[inline]
    fn has_toml_fields(&self) -> bool {
        self.binary_name.is_some() && self.version.is_some()
    }

    fn conditionally_parse_toml(&mut self) -> io::Result<()> {
        if self.has_toml_fields() {
            return Ok(());
        }

        fn try_parse_field(line: &str, field: &'static str) -> Option<String> {
            line.strip_prefix(field)
                .map(|rest| rest.trim_matches([' ', '\'', '\"', '=']).to_string())
        }

        let toml = fs::File::open(self.project_dir.join("Cargo.toml"))?;
        let reader = BufReader::new(toml);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim_start();

            if let Some(Some(name)) = self
                .binary_name
                .is_none()
                .then(|| try_parse_field(line, "name"))
            {
                self.binary_name = Some(name)
            } else if let Some(Some(version_str)) = self
                .version
                .is_none()
                .then(|| try_parse_field(line, "version"))
            {
                self.version = Some(version_str)
            }

            if self.has_toml_fields() {
                break;
            }
        }

        if !self.has_toml_fields() {
            exit_err!("Failed to parse Cargo.toml")
        }

        if self.dry_run {
            println!("Parsed Cargo.toml")
        }

        Ok(())
    }
}

impl Variables {
    fn from(mut args: Args) -> io::Result<Self> {
        args.conditionally_parse_toml()?;

        let binary_name = args
            .binary_name
            .expect("`conditionally_parse_toml` will return early before this is `None`");
        Ok(Self {
            project_dir: args.project_dir,
            linux_binary_name: binary_name.replace('_', "-"),
            binary_name,
            version: args
                .version
                .expect("`conditionally_parse_toml` will return early before this is `None`"),
            architecture: args.architecture,
        })
    }

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

        if !file_type.is_text() {
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

        output.flush()
    }
}

trait DebCollector {
    fn conditional_insert(&mut self, entry: &DirEntry, dry_run: bool);
}

impl DebCollector for DebFiles {
    fn conditional_insert(&mut self, entry: &DirEntry, dry_run: bool) {
        if let Some(deb_file) = entry.debian_file() {
            if self.insert(deb_file, entry.path()).is_some() {
                exit_err!("Found more than 1 {deb_file:?} file")
            }
            if dry_run {
                println!("Found {deb_file:?} file")
            }
        }
    }
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

    fn scan<P>(self, directory: P, deb_files: &mut DebFiles, dry_run: bool) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            match self {
                SearchDir::Assets | SearchDir::Debian if file_type.is_dir() => {
                    self.scan(entry.path(), deb_files, dry_run)?
                }
                SearchDir::Build if file_type.is_dir() => {
                    let file_name = entry.file_name();
                    if !dry_run && file_name == TEMP_DIR {
                        fs::remove_dir_all(entry.path())?;
                        println!("Reset contents of ~\\build\\tmp")
                    } else if file_name == SearchDir::Debian.name() {
                        SearchDir::Debian.scan(entry.path(), deb_files, dry_run)?
                    }
                }
                _ if file_type.is_file() => deb_files.conditional_insert(&entry, dry_run),
                _ => (),
            }
        }
        Ok(())
    }
}

impl Forge {
    pub fn from(args: Args) -> io::Result<Self> {
        let dry_run = args.dry_run;
        let vars = Variables::from(args)?;

        let mut deb_files = HashMap::new();

        let binary_path = vars.get_binary_path();
        if binary_path.exists() {
            deb_files.insert(FileType::Binary, binary_path);
            if dry_run {
                println!("Found Binary file")
            }
        } else {
            exit_err!("Failed to find a binary at {}", binary_path.display())
        }

        for entry in fs::read_dir(&vars.project_dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let file_name = entry.file_name();
                if let Some(search_dir) = SEARCH_DIRS.iter().find(|valid| file_name == valid.name())
                {
                    search_dir.scan(entry.path(), &mut deb_files, dry_run)?;
                }
            } else if file_type.is_file() {
                deb_files.conditional_insert(&entry, dry_run)
            }
        }

        for required in REQUIRED_DEB_FILES.iter() {
            if !deb_files.contains_key(required) {
                exit_err!("Could not locate a {required:?} file")
            }
        }

        if dry_run {
            println!("{PKG_NAME}: Success valid project file structure");
            std::process::exit(0)
        }

        Ok(Self {
            vars,
            files: deb_files,
        })
    }

    pub fn forge(self) -> io::Result<()> {
        for (&file, path) in self.files.iter() {
            self.vars.write_file(file, path)?
        }

        println!(
            "{PKG_NAME}: Successfully imported {} files, and project binary",
            self.files.len() - 1
        );
        Ok(())
    }
}
