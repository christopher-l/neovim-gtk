mod buffer_list;
mod file_browser;

use std::ops::Deref;

use gtk;

use shell;
use self::buffer_list::BufferList;
use self::file_browser::FileBrowser;

pub struct SidePanelWidget {
    widget: gtk::Box,
    buffer_list: BufferList,
    file_browser: FileBrowser,
}

impl SidePanelWidget {
    pub fn new() -> Self {
        let builder = gtk::Builder::new_from_string(include_str!("../../resources/side-panel.ui"));
        let widget: gtk::Box = builder.get_object("side_panel").unwrap();
        let buffer_list = BufferList::new(&builder);
        let file_browser = FileBrowser::new(&builder);
        SidePanelWidget {
            widget,
            buffer_list,
            file_browser,
        }
    }

    pub fn init(&mut self, shell_state: &shell::State) {
        self.buffer_list.init(&shell_state);
        self.file_browser.init(&shell_state);
    }
}

impl Deref for SidePanelWidget {
    type Target = gtk::Box;

    fn deref(&self) -> &gtk::Box {
        &self.widget
    }
}
