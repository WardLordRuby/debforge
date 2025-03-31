use std::{
    borrow::Cow,
    fs::DirEntry,
    path::{Path, PathBuf},
};

use super::Variables;

const ICON_FORMATS: [&str; 5] = ["png", "jpg", "jpeg", "tiff", "svg"];
const ICONS: [FileType; 4] = [
    FileType::Icon64,
    FileType::Icon128,
    FileType::Icon256,
    FileType::Icon512,
];

pub(super) trait CowExt {
    fn as_path(&self) -> &Path;
}

impl CowExt for Cow<'_, str> {
    fn as_path(&self) -> &Path {
        match self {
            &Cow::Borrowed(str) => Path::new(str),
            Cow::Owned(str) => Path::new(str),
        }
    }
}

pub(super) trait DebParser {
    fn debian_file(&self) -> Option<FileType>;
}

impl DebParser for &DirEntry {
    fn debian_file(&self) -> Option<FileType> {
        let file_name = self.file_name();
        let name_str = file_name.to_str()?;

        if let Some((_, extension)) = name_str.rsplit_once('.') {
            if extension == "desktop" {
                return Some(FileType::Desktop);
            }

            if ICON_FORMATS.iter().any(|&fmt| extension == fmt) {
                return ICONS
                    .iter()
                    .find(|&icon| name_str.contains(icon.width()))
                    .copied();
            }

            return None;
        }

        FileType::from(name_str)
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub(super) enum FileType {
    // required
    Control,
    Changelog,
    Copyright,
    Binary,

    // optional
    Icon64,
    Icon128,
    Icon256,
    Icon512,
    Desktop,
    Install,
    PreInst,
    PostInst,
    PreRm,
    PostRm,
    ConfFiles,
    Watch,
    Format,
    Dirs,
    Docs,
    Menu,
    ManPages,
}

impl FileType {
    fn from(str: &str) -> Option<Self> {
        Some(match str {
            "control" => FileType::Control,
            "changelog" => FileType::Changelog,
            "copyright" => FileType::Copyright,
            "install" => FileType::Install,
            "preinst" => FileType::PreInst,
            "postinst" => FileType::PostInst,
            "prerm" => FileType::PreRm,
            "postrm" => FileType::PostRm,
            "conffiles" => FileType::ConfFiles,
            "watch" => FileType::Watch,
            "format" => FileType::Format,
            "dirs" => FileType::Dirs,
            "docs" => FileType::Docs,
            "desktop" => FileType::Desktop,
            "menu" => FileType::Menu,
            "manpages" => FileType::ManPages,
            _ => return None,
        })
    }

    pub(super) fn is_text(self) -> bool {
        !matches!(
            self,
            FileType::Icon64
                | FileType::Icon128
                | FileType::Icon256
                | FileType::Icon512
                | FileType::Binary
        )
    }

    fn width(self) -> &'static str {
        match self {
            FileType::Icon64 => "64",
            FileType::Icon128 => "128",
            FileType::Icon256 => "256",
            FileType::Icon512 => "512",
            _ => unreachable!("Only icons have widths"),
        }
    }

    fn resolution(self) -> &'static str {
        match self {
            FileType::Icon64 => "64x64",
            FileType::Icon128 => "128x128",
            FileType::Icon256 => "256x256",
            FileType::Icon512 => "512x512",
            _ => unreachable!("Only icons have resolutions"),
        }
    }

    pub(super) fn output_file_name(self, linux_binary_name: &str) -> Cow<'_, str> {
        match self {
            FileType::Control => Cow::Borrowed("control"),
            FileType::Changelog => Cow::Borrowed("changelog"),
            FileType::Copyright => Cow::Borrowed("copyright"),
            FileType::Binary => Cow::Borrowed(linux_binary_name),
            FileType::Icon64 | FileType::Icon128 | FileType::Icon256 | FileType::Icon512 => {
                Cow::Owned(format!("{linux_binary_name}.png"))
            }
            FileType::Desktop => Cow::Owned(format!("{linux_binary_name}.desktop")),
            FileType::Install => Cow::Borrowed("install"),
            FileType::PreInst => Cow::Borrowed("preinst"),
            FileType::PostInst => Cow::Borrowed("postinst"),
            FileType::PreRm => Cow::Borrowed("prerm"),
            FileType::PostRm => Cow::Borrowed("postrm"),
            FileType::ConfFiles => Cow::Borrowed("conffiles"),
            FileType::Watch => Cow::Borrowed("watch"),
            FileType::Format => Cow::Borrowed("format"),
            FileType::Dirs => Cow::Borrowed("dirs"),
            FileType::Docs => Cow::Borrowed("docs"),
            FileType::Menu => Cow::Borrowed("menu"),
            FileType::ManPages => Cow::Borrowed("manpages"),
        }
    }
}

impl Variables {
    /// Binary source path
    pub(super) fn get_binary_path(&self) -> PathBuf {
        let mut out = self.project_dir.join(self.architecture.platform_bin_path());
        out.push(&self.binary_name);
        out
    }

    /// Output paths
    pub(super) fn get_file_type_path(&self, file_type: FileType) -> PathBuf {
        let mut out = self.project_dir.join(format!(
            "build/tmp/dist/linux/{}-{}",
            self.linux_binary_name, self.version
        ));

        match file_type {
            FileType::Changelog | FileType::Copyright => {
                out.push(format!("usr/share/doc/{}", self.linux_binary_name))
            }
            icon @ (FileType::Icon64
            | FileType::Icon128
            | FileType::Icon256
            | FileType::Icon512) => out.push(format!(
                "usr/share/icons/hicolor/{}/apps",
                icon.resolution()
            )),
            FileType::Binary => {
                out.push("usr/local/bin");
            }
            FileType::Desktop => out.push("usr/share/applications"),
            FileType::Format => out.push("DEBIAN/source"),
            _ => out.push("DEBIAN"),
        }
        out
    }
}
