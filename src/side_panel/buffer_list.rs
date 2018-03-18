use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use glib;
use gtk;
use gtk::prelude::*;

use neovim_lib::{NeovimApi, NeovimApiAsync, Value};

use aux::{get_buffer_name};
use nvim::{ErrorReport, NeovimClient};
use shell;
use ui::UiMutex;

#[derive(Debug, Default, PartialEq)]
struct Buffer {
    filename: String,
    number: u64,
    // changed: bool,
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
    buffers: Rc<RefCell<Vec<Buffer>>>,
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
            buffers: Rc::new(RefCell::new(Vec::new())),
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
        let stored_buffers_ref = &self.buffers;

        shell_state.subscribe(
            "BufAdd",
            &["getcwd()", "getbufinfo()"],
            clone!(list, paned, nvim_ref, pane_state_ref, stored_buffers_ref => move |args| {
                println!("BufAdd");
                let cwd = &args[0];
                let cwd = Path::new(cwd.as_str().unwrap());
                let buf_info = &args[1];
                let buffers = read_buffer_list(buf_info);
                on_buf_add(
                    &list,
                    &paned,
                    &nvim_ref,
                    &mut pane_state_ref.borrow_mut(),
                    &cwd,
                    buffers,
                    &mut stored_buffers_ref.borrow_mut(),
                );
            }),
        );

        shell_state.subscribe(
            "BufDelete",
            &[],
            clone!(nvim_ref, list, paned, pane_state_ref, stored_buffers_ref => move |_| {
                println!("BufDelete");
                // BufDelete is triggered before the buffer is deleted, so wait for a cycle.
                let mut data = Some(UiMutex::new((
                    list.clone(),
                    paned.clone(),
                    pane_state_ref.clone(),
                    stored_buffers_ref.clone(),
                )));
                nvim_ref.nvim().unwrap().eval_async("getbufinfo()").cb(move |value| {
                    glib::idle_add(move || {
                        match value {
                            Ok(ref buf_info) => {
                                let data = data.take().unwrap();
                                let (
                                    ref list,
                                    ref paned,
                                    ref pane_state_ref,
                                    ref stored_buffers_ref
                                ) = *data.borrow_mut();
                                let buffers = read_buffer_list(buf_info);
                                on_buf_delete(
                                    &list,
                                    &paned,
                                    &mut pane_state_ref.borrow_mut(),
                                    &buffers,
                                    &mut stored_buffers_ref.borrow_mut(),
                                );
                            }
                            Err(ref err) => error!("Couldn't get bufinfo: {}", err),
                        }
                        glib::Continue(false)
                    });
                }).call();
            }),
        );

        shell_state.subscribe(
            "BufEnter",
            &["bufnr('%')"],
            clone!(list, stored_buffers_ref => move |args| {
                println!("BufEnter");
                let current_buffer_number = args[0].as_u64().unwrap();
                on_buf_enter(
                    &list,
                    &stored_buffers_ref.borrow(),
                    current_buffer_number,
                );
            }),
        );

        shell_state.subscribe(
            "DirChanged",
            &["getcwd()", "getbufinfo()"],
            clone!(list => move |args| {
                let cwd = &args[0];
                let cwd = Path::new(cwd.as_str().unwrap());
                let buf_info = &args[1];
                let buffers = read_buffer_list(buf_info);
                on_dir_changed(
                    &list,
                    &cwd,
                    &buffers,
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
                nvim_ref,
                &mut pane_state_ref.borrow_mut(),
                &cwd,
                &buffers,
            );
            *stored_buffers_ref.borrow_mut() = buffers;
        }
    }

    fn connect_events(&mut self) {
        let list = &self.list;
        let nvim_ref = self.nvim.as_ref().unwrap();
        let stored_buffers_ref = &self.buffers;

        list.connect_row_activated(clone!(nvim_ref, stored_buffers_ref => move |list, row| {
            if let Some(index) = list.get_children().iter().position(|r| r == row) {
                if let Some(buffer) = stored_buffers_ref.borrow().get(index) {
                    let mut nvim = nvim_ref.nvim().unwrap();
                    nvim.command_async(&format!(":b {}", buffer.number))
                        .cb(|r| r.report_err())
                        .call();
                }
            }
        }));
    }
}

fn init_list(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    nvim_ref: &Rc<NeovimClient>,
    pane_state: &mut PaneState,
    cwd: &Path,
    buffers: &[Buffer],
) {
    let n_buffers = buffers.len();
    for buffer in buffers {
        add_row(list, Rc::clone(nvim_ref), buffer, cwd);
    }
    update_pane_position(paned, pane_state, n_buffers);
}

fn on_buf_add(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    nvim_ref: &Rc<NeovimClient>,
    pane_state: &mut PaneState,
    cwd: &Path,
    buffers: Vec<Buffer>,
    stored_buffers: &mut Vec<Buffer>,
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    if let Some(buffer) = buffers.into_iter().find(|buffer| {
        !stored_buffers.iter().any(|stored_buffer| stored_buffer == buffer)
    }) {
        add_row(list, Rc::clone(nvim_ref), &buffer, cwd);
        stored_buffers.push(buffer);
    } else {
        error!("Empty buffer list after BufAdd was received.");
    }
    if !pane_state.was_dragged {
        update_pane_position(paned, pane_state, rows.len() + 1);
    }
}

fn on_buf_delete(
    list: &gtk::ListBox,
    paned: &gtk::Paned,
    pane_state: &mut PaneState,
    buffers: &[Buffer],
    stored_buffers: &mut Vec<Buffer>,
) {
    let rows = list.get_children();
    if !pane_state.was_dragged {
        update_pane_was_dragged(paned, pane_state, &*rows);
    }
    let mut index = None;
    for (current_index, stored_buffer) in stored_buffers.iter().enumerate() {
        if !buffers.iter().any(|buffer| stored_buffer == buffer) {
            index = Some(current_index);
            break;
        }
    }
    if let Some(index) = index {
        list.remove(&rows[index]);
        stored_buffers.remove(index);
        if !pane_state.was_dragged {
            update_pane_position(paned, pane_state, rows.len() - 1);
        }
    } else {
        error!("Failed to remove deleted buffer from buffer list.")
    }
}

fn on_buf_enter(
    list: &gtk::ListBox,
    buffers: &[Buffer],
    current_buffer_number: u64,
) {
    if let Some(index) = buffers.iter().position(|buffer| buffer.number == current_buffer_number) {
        if let Some(row) = list.get_children().get(index) {
            let row = row.clone().downcast::<gtk::ListBoxRow>().unwrap();
            list.select_row(&row);
        }
    }
}

fn on_dir_changed(
    list: &gtk::ListBox,
    cwd: &Path,
    buffers: &[Buffer],
) {
    let rows = list.get_children();
    for (row, buffer) in rows.iter().zip(buffers) {
        let label = get_label(row.clone());
        label.set_text(&get_buffer_name(&buffer.filename, cwd));
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

fn add_row(list: &gtk::ListBox, nvim_ref: Rc<NeovimClient>, buffer: &Buffer, cwd: &Path) {
    let builder = gtk::Builder::new_from_string(
        include_str!("../../resources/buffer_list_row.ui"),
    );
    let row: gtk::ListBoxRow = builder.get_object("row").unwrap();
    let label: gtk::Label = builder.get_object("label").unwrap();
    let close_btn: gtk::Button = builder.get_object("close_btn").unwrap();
    label.set_label(&get_buffer_name(&buffer.filename, cwd));
    list.add(&row);
    row.show();
    let buffer_number = buffer.number;
    close_btn.connect_clicked(move |_| {
        let mut nvim = nvim_ref.nvim().unwrap();
        nvim.command_async(&format!(":bd {}", buffer_number))
            .cb(|r| r.report_err())
            .call();
    });
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
                "name" => buffer.filename = value.as_str().unwrap().to_owned(),
                "bufnr" => buffer.number = value.as_u64().unwrap(),
                // "changed" => buffer.changed = value.as_u64().unwrap() != 0,
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
