use std::fs::{self, DirEntry};
use std::os::unix::fs::PermissionsExt;

use dioxus::html::input_data::keyboard_types::Key;
use dioxus::prelude::*;
use dioxus_free_icons::icons::fi_icons;
use dioxus_free_icons::Icon;

#[derive(Debug)]
pub enum InputEvent {
    Down,
    Up,
    Confirm,
}

#[derive(Props)]
pub struct SelectBinaryProps<'a> {
    load: EventHandler<'a, String>,
    on_keyboard: Option<EventHandler<'a, InputEvent>>,
}

fn select_binary<'a>(cx: Scope<'a, SelectBinaryProps<'a>>) -> Element<'a> {
    let input_value_state = use_shared_state::<InputValue>(cx).unwrap();
    let input_value = input_value_state.read().0.clone();
    cx.render(rsx! {
        div {
            input {
                style: "margin-right: 10px;",
                value: "{input_value}",
                onkeydown: move |e| {
                    if e.inner().key() == Key::ArrowDown {
                        cx.props.on_keyboard.as_ref().unwrap_or(&cx.event_handler(|_| {})).call(InputEvent::Down);
                    } else if e.inner().key() == Key::ArrowUp {
                        cx.props.on_keyboard.as_ref().unwrap_or(&cx.event_handler(|_| {})).call(InputEvent::Up);
                    } else if e.inner().key() == Key::Enter {
                        cx.props.on_keyboard.as_ref().unwrap_or(&cx.event_handler(|_| {})).call(InputEvent::Confirm);
                    }
                },
                oninput: move |e| {
                    input_value_state.write().0 = e.value.clone();
                    // cx.props.on_input.as_ref().unwrap_or(&cx.event_handler(|_| {})).call(e.value.clone());
                },
            },
            button {
                onclick: move |_| cx.props.load.call(input_value.clone()),
                "Load"
            }
        }
    })
}

impl PartialEq for FuzzyFileSearchProps {
    fn eq(&self, other: &Self) -> bool {
        self.selection == other.selection && self.files.len() == other.files.len()
    }
}

#[derive(Clone)]
struct FileEntry {
    name: String,
    file_type: fs::FileType,
    metadata: fs::Metadata,
}

impl From<DirEntry> for FileEntry {
    fn from(value: DirEntry) -> Self {
        Self {
            name: value
                .path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            file_type: value.file_type().unwrap(),
            metadata: value.metadata().unwrap(),
        }
    }
}

#[derive(Props)]
struct FuzzyFileSearchProps {
    files: Vec<FileEntry>,
    input: String,
    selection: i32,
}

fn fuzzy_file_search<'a>(cx: Scope<'a, FuzzyFileSearchProps>) -> Element<'a> {
    let entries = cx
        .props
        .files
        .iter()
        .map(|f| (&f.name, &f.file_type, &f.metadata))
        .map(|(f, t, m)| {
            (
                f,
                if t.is_dir() {
                    rsx! {
                        Icon {
                            icon: fi_icons::FiFolder,
                        }
                    }
                } else if t.is_file() {
                    if m.permissions().mode() & 0o111 != 0 {
                        rsx! {
                            Icon {
                                icon: fi_icons::FiTerminal,
                            }
                        }
                    } else {
                        rsx! {
                            Icon {
                                icon: fi_icons::FiFileText,
                            }
                        }
                    }
                } else {
                    rsx! {
                        Icon {
                            icon: fi_icons::FiLink,
                        }
                    }
                },
            )
        })
        .enumerate()
        .map(|(i, f)| {
            (
                f.0,
                f.1,
                if *f.0 == cx.props.input {
                    "color: green;"
                } else if i as i32 == cx.props.selection {
                    "color: red;"
                } else {
                    ""
                },
            )
        })
        .map(|f| {
            rsx! {
                li {
                    style: f.2,
                    onclick: |e| println!("{:?}", e),
                    f.1,
                    span {
                        " ",
                    }
                    f.0.clone()
                }
            }
        });
    cx.render(rsx! {
        div {
            style: "display: flex; flex-direction: column;",
            ul {
                entries
            }
        }
    })
}

struct InputValue(String);

pub fn load_binary_view<'a>(cx: Scope<'a, SelectBinaryProps<'a>>) -> Element<'a> {
    use_shared_state_provider(cx, || InputValue("".to_owned()));
    let input_value_state = use_shared_state::<InputValue>(cx).unwrap();
    let input_value = input_value_state.read().0.clone();
    let mut selection: &UseState<i32> = use_state(cx, || -1);

    let search_term = input_value
        .rsplit_once("/")
        .unwrap_or((&input_value, &input_value));
    let files = fs::read_dir(if input_value.contains("/") {
        search_term.0
    } else {
        "./"
    })
    .unwrap();

    let files: Vec<FileEntry> = files
        .map(|f| f.unwrap())
        .filter(|f| {
            f.path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(search_term.1)
        })
        .map(|f| f.into())
        .collect();
    let file_count = files.len() as i32;
    let files_closure = files.clone();
    if **selection >= file_count {
        selection.set(file_count - 1);
    }
    cx.render(rsx! {
        div {
            style: "width: 100%; height: 100%; position: absolute; top: 0; left: 0; padding: 0; margin: 0; display: flex; flex-direction: column; justify-content: center; align-items: center; overflow: hidden;",
            select_binary {
                load: cx.event_handler(|e| cx.props.load.call(e)),
                on_keyboard: move |e| {
                    match e {
                        InputEvent::Down => {
                                selection += 1;
                        },
                        InputEvent::Up => {
                            if *selection.get() != -1 {
                                selection -= 1;
                            }
                        },
                        InputEvent::Confirm => {
                            if *selection.get() >= 0 && *selection.get() < file_count {
                                input_value_state.write().0 = files_closure[*selection.get() as usize].name.clone();
                            }
                        },
                    }
                }
            }
            fuzzy_file_search {
                files: files,
                input: input_value,
                selection: **selection,
            }
        }
    })
}
