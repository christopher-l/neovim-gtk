use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk;
use gtk::prelude::*;

use neovim_lib::NeovimApi;

use aux::{get_buffer_name, get_current_dir};
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

        shell_state.subscribe(
            "BufAdd",
            &["getcwd()"],
            clone!(list, paned, nvim_ref, pane_state_ref => move |args| {
                println!("BufAdd");
                let cwd = Path::new(&args[0]);
                on_add(
                    &list,
                    &paned,
                    &mut nvim_ref.nvim().unwrap(),
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                );
            }),
        );

        shell_state.subscribe(
            "BufDelete",
            &["getcwd()"],
            clone!(list, paned, nvim_ref, pane_state_ref => move |args| {
                println!("BufDelete");
                let cwd = Path::new(&args[0]);
                on_delete(
                    &list,
                    &paned,
                    &mut nvim_ref.nvim().unwrap(),
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                );
            }),
        );

        shell_state.subscribe(
            "DirChanged",
            &["getcwd()"],
            clone!(list, nvim_ref => move |args| {
                let cwd = Path::new(&args[0]);
                on_dir_changed(
                    &list,
                    &mut nvim_ref.nvim().unwrap(),
                    &cwd,
                );
            }),
        );

        let mut nvim = nvim_ref.nvim().unwrap();
        if let Some(cwd) = get_current_dir(&mut nvim) {
            init_list(
                &list,
                &paned,
                &mut nvim,
                &mut pane_state_ref.borrow_mut(),
                &Path::new(&cwd),
             )
        }
    }

    fn connect_events(&mut self) {
    }
}

fn init_list(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    nvim: &mut NeovimRef,
    pane_state: &mut PaneState,
    cwd: &Path,
) {
    let buffers = get_buffers(nvim, cwd);
    let n_buffers = buffers.len();
    for buffer_name in buffers {
        add_row(list, &buffer_name);
    }
    update_pane_position(paned, pane_state, n_buffers);
}

fn on_add(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    nvim: &mut NeovimRef,
    pane_state: &mut PaneState,
    cwd: &Path,
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    if let Some(new_buffer_name) = get_buffers(nvim, cwd).last() {
        add_row(list, new_buffer_name);
    } else {
        error!("Empty buffer list after BufAdd was received.");
    }
    if !pane_state.was_dragged {
        update_pane_position(paned, pane_state, rows.len() + 1);
    }
}

fn on_delete(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    nvim: &mut NeovimRef,
    pane_state: &mut PaneState,
    cwd: &Path,
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    let buffer_names = get_buffers(nvim, cwd);
    let mut removed_row = false;
    for (row, buffer_name) in rows.iter().zip(buffer_names) {
        let label = get_label(row.clone());
        if label.get_text() != Some(buffer_name) {
            list.remove(row);
            removed_row = true;
            break;
        }
    }
    if !removed_row {
        if let Some(last) = rows.last() {
            list.remove(last);
        }
    }
    if !pane_state.was_dragged {
        update_pane_position(paned, pane_state, rows.len() - 1);
    }
}

fn on_dir_changed(
    list: &gtk::ListBox,
    nvim: &mut NeovimRef,
    cwd: &Path,
) {
    let rows = list.get_children();
    let buffer_names = get_buffers(nvim, cwd);
    for (row, buffer_name) in rows.iter().zip(buffer_names) {
        let label = get_label(row.clone());
        label.set_text(&buffer_name);
    }
}

fn update_pane_was_dragged(
    paned: &gtk::Paned,
    pane_state: &mut PaneState,
    rows: &[gtk::Widget]
) {
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

fn update_pane_position(
    paned: &gtk::Paned,
    pane_state: &PaneState,
    n_buffers: usize,
) {
    let required_height = pane_state.row_height * n_buffers + pane_state.offset;
    if  required_height > pane_state.min_height {
        paned.set_position(required_height as i32);
    } else {
        paned.set_position(pane_state.min_height as i32);
    }
}

fn get_buffers(
    mut nvim: &mut NeovimRef,
    cwd: &Path,
) -> Vec<String> {
    let buffers = match nvim.list_bufs() {
        Ok(buffers) => buffers,
        Err(err) => {
            error!("Couldn't read buffer list: {}", err);
            return Vec::new();
        }
    };
    // buffers
    //     .iter()
    //     .filter_map(|buffer| {
    //         let is_listed = buffer
    //             .get_option(&mut nvim, "buflisted")
    //             .unwrap()
    //             .as_bool()
    //             .unwrap();
    //         if is_listed {
    //             buffer.get_name(&mut nvim).ok()
    //         } else {
    //             None
    //         }
    //     })
    //     .map(|filename| {
    //         get_buffer_name(&filename, cwd)
    //     })
    //     .collect()
    vec!["foo".to_owned()]
}

fn add_row(list: &gtk::ListBox, buffer_name: &str) {
    let builder = gtk::Builder::new_from_string(
        include_str!("../../resources/buffer_list_row.ui"),
    );
    let row: gtk::ListBoxRow = builder.get_object("row").unwrap();
    let label: gtk::Label = builder.get_object("label").unwrap();
    label.set_label(&buffer_name);
    list.add(&row);
    row.show();
}

fn get_label(row: gtk::Widget) -> gtk::Label {
    row
        .downcast::<gtk::ListBoxRow>()
        .unwrap()
        .get_children()
        .into_iter()
        .next()
        .unwrap()
        .downcast::<gtk::Box>()
        .unwrap()
        .get_children()
        .into_iter()
        .nth(1)
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap()
}
