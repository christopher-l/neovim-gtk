mod buffer_list;
mod file_browser;

use std::cmp::max;
use std::ops::Deref;

use gtk;
use gtk::prelude::*;

use shell;
use self::buffer_list::BufferList;
use self::file_browser::FileBrowser;

pub struct SidePanelWidget {
    builder: gtk::Builder,
    widget: gtk::Box,
    paned: gtk::Paned,
    buffer_list: BufferList,
    file_browser: FileBrowser,
}

impl SidePanelWidget {
    pub fn new() -> Self {
        let builder = gtk::Builder::new_from_string(include_str!("../../resources/side-panel.ui"));
        let widget: gtk::Box = builder.get_object("side_panel").unwrap();
        let paned: gtk::Paned = builder.get_object("paned").unwrap();
        let buffer_list = BufferList::new(&builder);
        let file_browser = FileBrowser::new(&builder);
        SidePanelWidget {
            builder,
            widget,
            paned,
            buffer_list,
            file_browser,
        }
    }

    pub fn init(&mut self, shell_state: &shell::State) {
        self.buffer_list.init(&shell_state);
        self.file_browser.init(&shell_state);
        self.connect_events();
    }

    fn connect_events(&mut self) {
        let buffer_list_scroll: gtk::ScrolledWindow =
            self.builder.get_object("buffer_list_scroll").unwrap();
        let buffer_list_box: gtk::Box =
            self.builder.get_object("buffer_list_box").unwrap();
        let offset =
            buffer_list_box.get_allocated_height() - buffer_list_scroll.get_allocated_height();
        let paned = &self.paned;
        let _resize_handler = self.buffer_list.list.connect_size_allocate(
            clone!(paned => move |_, alloc| {
                println!("foo");
                let new_height = alloc.height + offset;
                gtk::idle_add(clone!(paned => move || {
                    // paned.set_position(new_height);
                    gtk::Continue(false)
                }));
            }),
        );

        let list = &self.buffer_list.list;
        buffer_list_scroll.connect_size_allocate(clone!(list => move |scroll, alloc| {
        //     let offset = list.get_allocated_height() - alloc.height;
        //     println!("offset: {:?}", offset);
            println!("bar");
        }));

        // paned.connect_button_release_event(clone!(list, buffer_list_scroll => move |_, _| {
        //     let offset = list.get_allocated_height() - buffer_list_scroll.get_allocated_height();
        //     println!("offset: {:?}", offset);
        //     gtk::Inhibit(false)
        // }));
    }
}

impl Deref for SidePanelWidget {
    type Target = gtk::Box;

    fn deref(&self) -> &gtk::Box {
        &self.widget
    }
}
