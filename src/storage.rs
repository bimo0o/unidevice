use crate::error::{Error, Result};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub fn root() -> Result<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .ok_or(Error::Io("application directory".into()))
}
pub fn path(folder: &str, name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(Error::Invalid(
            "use letters, digits, - or _ for file name".into(),
        ));
    }
    Ok(root()?.join(folder).join(format!("{name}.json")))
}
pub fn read(path: &Path) -> Result<String> {
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|_| Error::Io("cannot open JSON file".into()))?
        .take(16385)
        .read_to_string(&mut text)
        .map_err(|_| Error::Io("cannot read JSON file".into()))?;
    if text.len() > 16384 {
        return Err(Error::Invalid("JSON exceeds 16KiB".into()));
    }
    Ok(text)
}
pub fn save_new(path: &Path, text: &str) -> Result<()> {
    if text.len() > 16384 {
        return Err(Error::Invalid("JSON exceeds 16KiB".into()));
    }
    let parent = path.parent().ok_or(Error::Io("file directory".into()))?;
    std::fs::create_dir_all(parent).map_err(|_| Error::Io("cannot create directory".into()))?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Error::Io("file already exists or cannot be created".into()))?;
    f.write_all(text.as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|_| Error::Io("cannot save file".into()))
}
