use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk;
use gtk::prelude::*;

use neovim_lib::NeovimApi;

use nvim::{NeovimClient, NeovimRef};
use shell;

struct PaneState {
    min_height: usize,
    offset: usize,
    row_height: usize,
    was_dragged: bool,
}

pub struct BufferList {
    builder: gtk::Builder,
    list: gtk::ListBox,
    paned: gtk::Paned,
    nvim: Option<Rc<NeovimClient>>,
    pane_state: Rc<RefCell<PaneState>>,
}

impl BufferList {
    pub fn new(builder: &gtk::Builder) -> Self {
        let list: gtk::ListBox = builder.get_object("buffer_list").unwrap();
        let paned: gtk::Paned = builder.get_object("paned").unwrap();
        Self {
            builder: builder.clone(),
            list,
            paned,
            nvim: None,
            pane_state: Rc::new(RefCell::new(PaneState {
                min_height: 0,
                offset: 0,
                row_height: 0,
                was_dragged: false,
            })),
        }
    }

    pub fn init(&mut self, shell_state: &shell::State) {
        let nvim = shell_state.nvim_clone();
        self.nvim = Some(nvim);

        self.init_pane_state();
        self.init_subscriptions(&shell_state);
        self.connect_events();
    }

    fn init_pane_state(&mut self) {
        let pane_state = &mut self.pane_state.borrow_mut();
        pane_state.min_height = self.paned.get_position() as usize;
        let buffer_list_scroll: gtk::ScrolledWindow =
            self.builder.get_object("buffer_list_scroll").unwrap();
        let buffer_list_box: gtk::Box =
            self.builder.get_object("buffer_list_box").unwrap();
        pane_state.offset = buffer_list_box.get_allocated_height() as usize
            - buffer_list_scroll.get_allocated_height() as usize;
    }

    fn init_subscriptions(&mut self, shell_state: &shell::State) {
        let list = &self.list;
        let paned = &self.paned;
        let nvim_ref = self.nvim.as_ref().unwrap();
        let pane_state_ref = &self.pane_state;
        let update_list = shell_state.subscribe(
            "DirChanged",
            &["getcwd()"],
            clone!(list, paned, nvim_ref, pane_state_ref => move |args| {
                let cwd = Path::new(&args[0]);
                populate_list(
                    &list,
                    &paned,
                    &mut nvim_ref.nvim().unwrap(),
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                );
            }),
        );
        shell_state.run_now(&update_list);
    }

    fn connect_events(&mut self) {
    }
}

fn populate_list(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    mut nvim: &mut NeovimRef,
    pane_state: &mut PaneState,
    cwd: &Path,
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        if pane_state.row_height == 0 {
            if let Some(row) = rows.get(0) {
                pane_state.row_height = row.get_allocated_height() as usize;
            }
        }
        let required_height = pane_state.row_height * rows.len() + pane_state.offset;
        if (
            required_height > pane_state.min_height
            && paned.get_position() as usize != required_height
        ) || (
            required_height <= pane_state.min_height
            && paned.get_position() as usize > pane_state.min_height
        ) {
            pane_state.was_dragged = true;
        }
    }
    for widget in rows {
        list.remove(&widget);
    }
    let mut n_buffers = 0;
    if let Ok(buffers) = nvim.list_bufs() {
        for buffer in buffers {
            let is_listed = buffer
                .get_option(&mut nvim, "buflisted")
                .unwrap()
                .as_bool()
                .unwrap();
            if let (true, Ok(name)) = (is_listed, buffer.get_name(&mut nvim)) {
                n_buffers += 1;
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
    if !pane_state.was_dragged {
        let required_height = pane_state.row_height * n_buffers + pane_state.offset;
        if  required_height > pane_state.min_height {
            paned.set_position(required_height as i32);
        } else {
            paned.set_position(pane_state.min_height as i32);
        }
    }
}
