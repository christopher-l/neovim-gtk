use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk;
use gtk::prelude::*;

use neovim_lib::{NeovimApi, Value};

use aux::{get_buffer_name, get_current_dir};
use nvim::{NeovimClient, NeovimRef};
use shell;

#[derive(Debug, Default)]
struct Buffer {
    name: String,
    number: u64,
    changed: bool,
}

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
            &["getcwd()", "getbufinfo()"],
            clone!(list, paned, pane_state_ref => move |args| {
                println!("BufAdd");
                let cwd = &args[0];
                let cwd = Path::new(cwd.as_str().unwrap());
                let buf_info = &args[1];
                let buffers = read_buffer_list(buf_info);
                on_add(
                    &list,
                    &paned,
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                    &buffers,
                );
            }),
        );

        shell_state.subscribe(
            "BufDelete",
            &["getcwd()", "getbufinfo()"],
            clone!(list, paned, pane_state_ref => move |args| {
                println!("BufDelete");
                let cwd = &args[0];
                let cwd = Path::new(cwd.as_str().unwrap());
                let buf_info = &args[1];
                let buffers = read_buffer_list(buf_info);
                on_delete(
                    &list,
                    &paned,
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                    &buffers,
                );
            }),
        );

        shell_state.subscribe(
            "DirChanged",
            &["getcwd()"],
            clone!(list, nvim_ref => move |args| {
                let cwd = &args[0];
                let cwd = Path::new(cwd.as_str().unwrap());
                on_dir_changed(
                    &list,
                    &mut nvim_ref.nvim().unwrap(),
                    &cwd,
                );
            }),
        );

        let mut nvim = nvim_ref.nvim().unwrap();
        if let Some(args) = ["getcwd()", "getbufinfo()"]
            .iter()
            .map(|arg| nvim.eval(arg))
            .map(|res| res.ok())
            .collect::<Option<Vec<Value>>>()
        {
            let cwd = &args[0];
            let cwd = Path::new(cwd.as_str().unwrap());
            let buf_info = &args[1];
            let buffers = read_buffer_list(buf_info);
            init_list(
                &list,
                &paned,
                &mut pane_state_ref.borrow_mut(),
                &cwd,
                &buffers,
             )
        }
    }

    fn connect_events(&mut self) {
    }
}

fn init_list(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    pane_state: &mut PaneState,
    cwd: &Path,
    buffers: &[Buffer],
) {
    let n_buffers = buffers.len();
    for buffer in buffers {
        add_row(list, buffer, cwd);
    }
    update_pane_position(paned, pane_state, n_buffers);
}

fn on_add(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    pane_state: &mut PaneState,
    cwd: &Path,
    buffers: &[Buffer],
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    if let Some(buffer) = buffers.last() {
        add_row(list, buffer, cwd);
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
    pane_state: &mut PaneState,
    cwd: &Path,
    buffers: &[Buffer],
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    let mut removed_row = false;
    for (row, buffer) in rows.iter().zip(buffers) {
        let label = get_label(row.clone());
        if label.get_text() != Some(get_buffer_name(&buffer.name, cwd)) {
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
    // let buffer_names = get_buffers(nvim, cwd);
    // for (row, buffer_name) in rows.iter().zip(buffer_names) {
    //     let label = get_label(row.clone());
    //     label.set_text(&buffer_name);
    // }
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

fn add_row(list: &gtk::ListBox, buffer: &Buffer, cwd: &Path) {
    let builder = gtk::Builder::new_from_string(
        include_str!("../../resources/buffer_list_row.ui"),
    );
    let row: gtk::ListBoxRow = builder.get_object("row").unwrap();
    let label: gtk::Label = builder.get_object("label").unwrap();
    label.set_label(&get_buffer_name(&buffer.name, cwd));
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

fn read_buffer_list(buf_info: &Value) -> Vec<Buffer> {
    let mut buffers = Vec::new();
    'buffer: for entry in buf_info.as_array().unwrap() {
        let map = entry.as_map().unwrap();
        let mut buffer = Buffer::default();
        for &(ref key, ref value) in map {
            match key.as_str().unwrap() {
                "name" => buffer.name = value.as_str().unwrap().to_owned(),
                "bufnr" => buffer.number = value.as_u64().unwrap(),
                "changed" => buffer.changed = value.as_u64().unwrap() != 0,
                "listed" => {
                    if value.as_u64().unwrap() != 1 {
                        continue 'buffer;
                    }
                }
                _ => {}
            }
        }
        buffers.push(buffer);
    }
    buffers
}
