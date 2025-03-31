# debforge
debforge is a command line tool that scans a project directory for common Deb package files automating the process
of changing target architecture and writing modified files into the correct file structure.

### Arguments
| Argument          | Short | Description                                             | Default                  |
| ----------------- | ----- | ------------------------------------------------------- | ------------------------ |
| `--package-dir`   | `-p`  | Optionally force a specific project directory           | current directory        |
| `--binary-name`   | `-b`  | Optionally supply a binary name                         | taken from Cargo.toml    |
| `--version`       | `-v`  | Optionally supply a binary version                      | taken from Cargo.toml    |
| `--target`        | `-t`  | Optionally supply the target architecture [amd, arm]    | x86_64-unknown-linux-gnu |
| `--dry-run`       | `-d`  | Run the program in dry mode, lists found debian files   | not enabled              |

### Searched paths
The project directory will be located using the current directory, this can be overridden by specifying a
`--package-dir`. Relative paths will be appended to the current directory.  

debforge will search the following directories for relevant debian files:
| Directory                      | Recursive? | Description                                                     |
| ------------------------------ | ---------- | --------------------------------------------------------------- |
| current dir or `--package-dir` | ❌         | Looks for listed directories, `Cargo.toml`, and debian files    |
| `~/build/`                     | ❌         | Searches all files and looks for the debian directory           |
| `~/assets/`                    | ✅         | Searches all files and subdirectories for icon assets           |
| `~/debian/`                    | ✅         | Searches all files and subdirectories for debian files          |

### Supported variable names
| Variable                | Source                                                   |
| ----------------------- | -------------------------------------------------------- |
| $BinaryName             | command line input or parsed from Cargo.toml             |
| $LinuxBinaryName        | $BinaryName converted to kebab-case                      |
| $Version                | command line input or parsed from Cargo.toml             |
| $Target                 | command line input or default                            |
| $Architecture           | inferred from target [amd6, arm64]                       |
