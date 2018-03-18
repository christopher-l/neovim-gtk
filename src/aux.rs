use std::path::Path;

/// Returns a meaningful  name for a buffer.
pub fn get_buffer_title(filename: &str, cwd: &Path) -> String {
    if filename.is_empty() {
        "[No Name]".to_owned()
    } else if let Some(rel_path) = Path::new(&filename)
        .strip_prefix(&cwd)
        .ok()
        .and_then(|p| p.to_str())
    {
        rel_path.to_owned()
    } else {
        filename.to_owned()
    }
}
