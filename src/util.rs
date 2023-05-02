use std::{
    path::Path,
    ffi::OsStr,

};

pub fn get_basename(file: &str) -> String {
    Path::new(file)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_owned()
}
