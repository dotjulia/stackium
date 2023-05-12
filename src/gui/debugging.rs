use dioxus::prelude::*;

pub fn debugging_view(cx: Scope) -> Element {
    cx.render(rsx! {
        div {
            "debug"
        }
    })
}
