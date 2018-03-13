use std::path::Path;
use std::rc::Rc;

use gtk;
use gtk::prelude::*;

use neovim_lib::NeovimApi;

use nvim::{NeovimClient, NeovimRef};
use shell;

pub struct BufferList {
    list: gtk::ListBox,
    nvim: Option<Rc<NeovimClient>>,
}

impl BufferList {
    pub fn new(builder: &gtk::Builder) -> Self {
        let list: gtk::ListBox = builder.get_object("buffer_list").unwrap();
        Self {
            list,
            nvim: None,
        }
    }

    pub fn init(&mut self, shell_state: &shell::State) {
        let nvim = shell_state.nvim_clone();
        self.nvim = Some(nvim);

        self.init_subscriptions(&shell_state);
    }

    fn init_subscriptions(&mut self, shell_state: &shell::State) {
        let list = &self.list;
        let nvim_ref = self.nvim.as_ref().unwrap();
        let _update_list = shell_state.subscribe(
            "DirChanged",
            &["getcwd()"],
            clone!(list, nvim_ref => move |args| {
                let cwd = Path::new(&args[0]);
                populate_list(&list, &mut nvim_ref.nvim().unwrap(), &cwd);
            }),
        );
        // shell_state.run_now(&update_list);
    }
}

fn populate_list(
    list: &gtk::ListBox,
    mut nvim: &mut NeovimRef,
    cwd: &Path,
) {
    for widget in list.get_children() {
        list.remove(&widget);
    }
    if let Ok(buffers) = nvim.list_bufs() {
        for buffer in buffers {
            if let Ok(name) = buffer.get_name(&mut nvim) {
                let display_name = if name.is_empty() {
                    "[No Name]"
                } else if let Some(rel_path) = Path::new(&name)
                    .strip_prefix(&cwd)
                    .ok()
                    .and_then(|p| p.to_str())
                {
                    rel_path
                } else {
                    &name
                };
                println!("display_name: {:?}", display_name);
                let builder = gtk::Builder::new_from_string(
                    include_str!("../../resources/buffer_list_row.ui"),
                );
                let row: gtk::ListBoxRow = builder.get_object("row").unwrap();
                let label: gtk::Label = builder.get_object("label").unwrap();
                label.set_label(&display_name);
                list.add(&row);
                row.show();
            }
        }
    }
}

