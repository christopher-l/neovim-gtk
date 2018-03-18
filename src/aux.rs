use std::path::Path;

use neovim_lib::NeovimApi;

use nvim::NeovimRef;

pub fn get_current_dir(nvim: &mut NeovimRef) -> Option<String> {
    match nvim.eval("getcwd()") {
        Ok(cwd) => cwd.as_str().map(|s| s.to_owned()),
        Err(err) => {
            error!("Couldn't get cwd: {}", err);
            None
        }
    }
}

/// Returns a meaningful  name for a buffer.
pub fn get_buffer_name(filename: &str, cwd: &Path) -> String {
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
