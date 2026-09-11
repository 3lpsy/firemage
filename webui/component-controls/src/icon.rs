use dioxus::prelude::*;

#[component]
pub fn Icon(name: String, #[props(default = 18)] size: u32) -> Element {
    let path = match name.as_str() {
        "brand" => "M12 2 15 9 22 12 15 15 12 22 9 15 2 12 9 9Z",
        "vms" => "M4 4H20V10H4ZM4 14H20V20H4ZM7 7H7.01M7 17H7.01M11 7H17M11 17H17",
        "egress" => "M4 6h8M4 12h16M4 18h8m12-6-4-4m4 4-4 4",
        "networks" => "M12 8V13M5 16V13H19V16M9 3H15V8H9ZM2 16H8V21H2ZM16 16H22V21H16Z",
        "kernels" => {
            "M6 6H18V18H6ZM9 9H15V15H9ZM9 2V6M15 2V6M9 18V22M15 18V22M2 9H6M2 15H6M18 9H22M18 15H22"
        }
        "snapshots" => "M7 3H21V17H7ZM3 7V21H17M10 7H18M10 11H18",
        "assets" => "M5 3H14L19 8V21H5ZM14 3V8H19M8 12H16M8 16H16",
        "activity" => "M3 12H7L10 4 14 20 17 12H21",
        "external" => "M14 3H21V10M21 3L10 14M10 3H3V21H21V14",
        "tokens" => "M14 10A5 5 0 1 1 9 5 5 5 0 0 1 14 10ZM13 13 21 21M17 17 20 14M19 19 22 16",
        "secrets" => "M5 10H19V21H5ZM8 10V6A4 4 0 0 1 16 6V10M12 14V17",
        "users" => {
            "M16 21V19A4 4 0 0 0 12 15H6A4 4 0 0 0 2 19V21M22 21V19A4 4 0 0 0 19 15.13M16 3.13A4 4 0 0 1 16 10.87M13 7A4 4 0 1 1 5 7A4 4 0 0 1 13 7"
        }
        "config" => "M4 6H20M4 12H20M4 18H20M8 3V9M16 9V15M10 15V21",
        "host" => "M4 4H20V17H4ZM8 21H16M12 17V21M7 8H7.01M10 8H17M7 12H17",
        "logout" => "M9 4H4V20H9M10 12H21M17 8 21 12 17 16",
        "refresh" => {
            "M3 12A9 9 0 0 1 12 3C15 3 18 5 21 8M21 3V8H16M21 12A9 9 0 0 1 12 21C9 21 6 19 3 16M3 21V16H8"
        }
        "chevron-down" => "M6 9L12 15L18 9",
        "plus" => "M12 5V19M5 12H19",
        "copy" => "M8 8H21V21H8ZM16 8V3H3V16H8",
        "download" => "M12 3V16M7 11L12 16L17 11M4 16V21H20V16",
        "chevron-right" => "M9 6L15 12L9 18",
        "close" => "M6 6 18 18M6 18 18 6",
        _ => "M4 4H20V20H4Z",
    };
    rsx! {
        svg {
            width: size,
            height: size,
            view_box: "0 0 24 24",
            fill: if name == "brand" { "currentColor" } else { "none" },
            stroke: "currentColor",
            stroke_width: "1.6",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            "focusable": "false",
            path { d: path }
        }
    }
}
