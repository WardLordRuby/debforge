debforge is a command line tool that scans a project directory for common Deb package files automating the process
of changing target architecture and writing modified files into the correct file structure.

Project directory will be located using the current directory, this can be overridden by setting the environment
variable $PROJ_DIR='target/project/path'.

### Searched paths
debforge will search the following directories for relevant debian files:

| Directory                    | Recursive? | Description                                                |
| ---------------------------- | ---------- | ---------------------------------------------------------- |
| current dir or `$PROJ_DIR`   | ❌         | Only looks for listed directories and `Cargo.toml`         |
| `~/build/`                   | ❌         | Searches all files and looks for the debian directory      |
| `~/assets/`                  | ✅         | Searches all files and subdirectories for icon assets      |
| `~/debian/`                  | ✅         | Searches all files and subdirectories for deb files        |


### Supported variable names
| Variable                | Source                                                   |
| ----------------------- | -------------------------------------------------------- |
| $BinaryName             | command line input or parsed from Cargo.toml             |
| $LinuxBinaryName        | $BinaryName converted to kebab-case                      |
| $Version                | command line input or parsed from Cargo.toml             |
| $Target                 | command line input defaults to x86_64-unknown-linux-gnu  |
| $Architecture           | inferred from target [amd64(default), arm64]             |
