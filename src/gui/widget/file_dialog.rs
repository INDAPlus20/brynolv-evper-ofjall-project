use crate::{gui::display::{Color, Point, Rect, Window}, harddisk::fat32::FileInfo, svec::SVec, ps2_keyboard::{KeyEvent, Modifiers, KeyCode}};

use super::{Widget, Response, Event};


#[derive(PartialEq, Eq)]
pub enum Action {
    Save,
    Open
}

pub struct FileDialog<'a, const TYPE: Action> {
    size: Point,
    dirty: bool,
    invalidated: Rect,
    current_path: SVec<char, 128>,
    current_name: SVec<char,128>,
    current_dir_entries: Option<SVec<FileInfo, 32>>,
    file_contents: &'a mut [char],
    cursor: usize,
    name_scroll: usize,
    selected: usize,
}

impl<'a> FileDialog<'a, { Action::Save }> {
    pub fn new_save_dialog(path: Option<SVec<char, 128>>, file_contents: &'a mut [char]) -> Self {
        Self::with_path(path, file_contents)
    }

    pub const fn new_save_uninit(file_contents: &'a mut [char]) -> Self {
        Self::new(file_contents)
    }
}

impl<'a> FileDialog<'a, { Action::Open }> {
    pub fn new_open_dialog(path: Option<SVec<char, 128>>, file_contents: &'a mut [char]) -> Self {
        Self::with_path(path, file_contents)
    }

    pub const fn new_open_uninit(file_contents: &'a mut [char]) -> Self {
        Self::new(file_contents)
    }
}

impl<'a, const TYPE: Action> FileDialog<'a, TYPE> {
    pub fn dialog_type(&self) -> Action {
        TYPE
    }

    pub fn with_path(path: Option<SVec<char, 128>>, data: &'a mut [char]) -> Self {
        Self {
            size: Point::new(0, 0),
            dirty: true,
            invalidated: Rect::EMPTY,
            current_path: path.unwrap_or(SVec::new()),
            current_dir_entries: None,
            file_contents: data,
            current_name: SVec::new(),
            cursor: 0,
            name_scroll: 0,
            selected: 0
        }
    }

    pub const fn new(data: &'a mut [char]) -> Self {
        Self {
            size: Point::new(0, 0),
            dirty: true,
            invalidated: Rect::EMPTY,
            current_dir_entries: None,
            file_contents: data,
            current_path: SVec::new(),
            current_name: SVec::new(),
            cursor: 0,
            name_scroll: 0,
            selected: 0
        }
    }
}

impl<'a, const TYPE: Action> Widget for FileDialog<'a, TYPE> {
    type InitData = ();

    fn initialize(&mut self, size: Point, init_data: Self::InitData) {
        self.size = size;
        self.current_dir_entries = Some(unsafe {
            crate::harddisk::fat32::list_entries(&self.current_path)
        });
    }

    fn draw(&mut self, mut window: Window) {
        if !self.dirty { return; }

        let title_bar_height = 32;
        let path_entry_height = 24;
        let main_area_height = self.size.y - title_bar_height - path_entry_height;
        let title_bar_color = Color::grayscale(0x44);
        let path_entry_color = Color::grayscale(0x11);
        let background_color = Color::grayscale(0x22);
        let selected_color = Color::new(0x44, 0x44, 0x77);
        let text_color = Color::WHITE;
        let title = match TYPE {
            Action::Save => "Save File",
            Action::Open => "Open File"
        };

        let title_area = Rect::new(0, 0, self.size.x, title_bar_height);
        let entry_area = Rect::new(0, title_area.height, self.size.x, path_entry_height);
        let main_area = Rect::new(0, title_area.height + entry_area.height, self.size.x, main_area_height);

        window.draw_rect(Rect::intersection(title_area, self.invalidated), title_bar_color);
        window.draw_rect(Rect::intersection(entry_area, self.invalidated), path_entry_color);
        window.draw_rect(Rect::intersection(main_area, self.invalidated), background_color);

        let mut x = (self.size.x - title.chars().count() * 8) / 2;
        for c in title.chars() {
            window.draw_char(Point::new(x, 8), 1, c, text_color, title_bar_color, None);
            x += 8;
        }

        let mut x = 4;
        for c in self.current_name.get_slice().iter().skip(self.name_scroll).take((self.size.x - 8) / 8) {
            window.draw_char(Point::new(x, title_bar_height + 4), 1, *c, text_color, path_entry_color, None);
            x += 8;
        }

        if self.selected == 0 {
            window.draw_rect(Rect::new((self.cursor - self.name_scroll) * 8 + 4, title_bar_height + 4 + 16 - 4, 8, 3), background_color);
            window.draw_rect(Rect::new((self.cursor - self.name_scroll) * 8 + 4 + 1, title_bar_height + 4 + 16 - 4 + 1, 6, 1), text_color);
        }

        self.invalidated = Rect::EMPTY;
        self.dirty = false;
    }

    fn used_area(&self) -> crate::gui::display::Rect {
        Rect::new(0, 0, self.size.x, self.size.y)
    }

    fn invalidate(&mut self, area: Rect) {
        if self.invalidated.is_empty() {
            self.invalidated = area;
        } else {
            self.invalidated = Rect::smallest_containing(self.invalidated, area);
        }
        self.dirty = true;
    }

    fn dirty(&self) -> bool  {
        self.dirty
    }

    fn on_event(&mut self, event: Event) -> Response {
        match event {
            Event::KeyEvent(event) => match event {
                KeyEvent { keycode: KeyCode::Enter, modifiers: Modifiers::NONE, .. } => {
                    let name = if self.selected == 0 {
                        &self.current_name
                    } else {
                        todo!()
                    };
                    let mut data = SVec::<_, 4096>::new();
                    for c in self.file_contents.iter() {
                        let mut buf = [0; 4];
                        let s = c.encode_utf8(&mut buf);
                        for b in s.bytes() {
                            data.push(b);
                        }
                    }
                    unsafe {
                        crate::harddisk::fat32::write_file(name, data.get_slice_mut());
                    }
                    Response::RemoveMe
                },
                KeyEvent { char: Some(c), .. } => {
                    let gcursor = self.cursor - self.name_scroll;
                    if self.selected == 0 {
                        self.invalidate(Rect::new(
                            gcursor * 8 + 4,
                            32 + 4,
                            self.size.x - gcursor * 8 - 4,
                            16
                        ));
                        self.current_name.insert(self.cursor, c);
                        self.cursor += 1;
                        if self.cursor >= (self.size.x - 8) / 8 + self.name_scroll {
                            self.name_scroll += 1;
                        }
                    }
                    Response::Nothing
                },
                KeyEvent { keycode: KeyCode::Delete, modifiers: Modifiers::NONE, .. } => {
                    let gcursor = self.cursor - self.name_scroll;
                    if self.selected == 0 && self.cursor < self.current_name.len() {
                        self.invalidate(Rect::new(
                            gcursor * 8 + 4,
                            32 + 4,
                            self.size.x - gcursor * 8 - 4,
                            16
                        ));
                        self.current_name.remove(self.cursor);
                        self.dirty = true;
                    }
                    Response::Nothing
                },
                KeyEvent { keycode: KeyCode::Left, modifiers: Modifiers::NONE, .. } => {
                    let gcursor = self.cursor - self.name_scroll;
                    if self.selected == 0 && self.cursor > 0 {
                        self.invalidate(if gcursor == 0 {
                            Rect::new(
                                4,
                                32 + 4,
                                self.size.x - 8,
                                16
                            )
                        } else {
                            Rect::new(
                                (gcursor - 1) * 8 + 4,
                                32 + 4,
                                16,
                                16
                            )
                        });
                        self.cursor -= 1;
                        if self.cursor < self.name_scroll {
                            self.name_scroll -= 1;
                        }
                        self.dirty = true;
                    }
                    Response::Nothing
                },
                KeyEvent { keycode: KeyCode::Right, modifiers: Modifiers::NONE, .. } => {
                    let gcursor = self.cursor - self.name_scroll;
                    if self.selected == 0 && self.cursor < self.current_name.len() {
                        self.invalidate(if gcursor + 1 == (self.size.x - 8) / 8 + self.name_scroll {
                            Rect::new(
                                4,
                                32 + 4,
                                self.size.x - 8,
                                16
                            )
                        } else {
                            Rect::new(
                                gcursor * 8 + 4,
                                32 + 4,
                                16,
                                16
                            )
                        });
                        self.cursor += 1;
                        if self.cursor >= (self.size.x - 8) / 8 + self.name_scroll {
                            self.name_scroll += 1;
                        }
                        self.dirty = true;
                    }
                    Response::Nothing
                },
                KeyEvent { keycode: KeyCode::Backspace, modifiers: Modifiers::NONE, .. } => {
                    let gcursor = self.cursor - self.name_scroll;
                    if self.selected == 0 && self.cursor > 0 {
                        self.invalidate(Rect::new(
                            gcursor.saturating_sub(1) * 8 + 4,
                            32 + 4,
                            self.size.x - gcursor * 8 - 4,
                            16
                        ));
                        self.cursor -= 1;
                        self.current_name.remove(self.cursor);
                        if self.cursor < self.name_scroll {
                            self.name_scroll -= 1;
                        }
                        self.dirty = true;
                    }
                    Response::Nothing
                }
                KeyEvent { keycode: KeyCode::Escape, modifiers: Modifiers::NONE, .. } => Response::RemoveMe,
                _ => Response::Nothing
            },
            _ => Response::Nothing
        }
    }
}