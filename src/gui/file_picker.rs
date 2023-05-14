use std::fs;

use iced::{
    widget::{column, container, scrollable, text, text_input, Column},
    Color, Element, Length,
};

pub struct FilePicker {
    value: String,
    files: Vec<(String, bool)>, // name, is_binary
}

impl Default for FilePicker {
    fn default() -> Self {
        Self {
            value: String::default(),
            files: get_files_for_input(String::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FilePickerMessage {
    InputChanged(String),
    Load(String),
    Complete,
    Submit,
}

fn get_files_for_input(input: String) -> Vec<(String, bool)> {
    let (path, filename) = input.rsplit_once("/").unwrap_or((".", input.as_str()));
    fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap())
        .map(|e| {
            (
                e.path().file_name().unwrap().to_str().unwrap().to_string(),
                e.metadata().unwrap().is_file(),
            )
        })
        .filter(|(f, _)| f.starts_with(filename))
        .collect()
}

impl FilePicker {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn update(&mut self, message: FilePickerMessage) -> Option<FilePickerMessage> {
        match message {
            FilePickerMessage::InputChanged(v) => {
                self.files = get_files_for_input(v.clone());
                self.value = v;
                None
            }
            FilePickerMessage::Complete => {
                self.files = if self.files.len() > 0 {
                    self.value = self.files.first().unwrap().clone().0;
                    vec![self.files.first().unwrap().clone()]
                } else {
                    vec![]
                };
                None
            }
            FilePickerMessage::Submit => Some(FilePickerMessage::Load(self.value.clone())),
            _ => None,
        }
    }

    pub fn view(&self) -> Element<FilePickerMessage> {
        let files = scrollable(Column::with_children(
            self.files
                .iter()
                .map(|f| {
                    Into::<Element<FilePickerMessage>>::into(text(f.0.as_str()).style(Color::from(
                        if f.1 {
                            [0.1, 0.9, 0.1]
                        } else {
                            [0.2, 0.2, 0.2]
                        },
                    )))
                })
                .collect(),
        ));
        container(column(vec![
            text_input("", self.value.as_str())
                .on_input(FilePickerMessage::InputChanged)
                .on_submit(FilePickerMessage::Submit)
                .into(),
            files.into(),
        ]))
        .height(Length::Fill)
        .width(Length::Fill)
        .center_y()
        .center_x()
        .padding(20)
        .into()
    }
}
