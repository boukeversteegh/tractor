//! Shared helpers used across multiple format renderers.

use std::path::Path;
use tractor_core::normalize_path;

pub fn to_absolute_path(path: &str) -> String {
    let p = Path::new(path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(p)
    } else {
        p.to_path_buf()
    };
    normalize_path(&absolute.to_string_lossy())
}
