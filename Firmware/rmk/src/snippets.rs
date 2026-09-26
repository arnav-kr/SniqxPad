use crate::hid::type_str;
use crate::set_alert;
use core::sync::atomic::AtomicU8;

pub static SELECTED_SNIPPET: AtomicU8 = AtomicU8::new(0);

#[derive(Clone)]
pub struct SnippetItem {
    pub name: heapless::String<16>,
    pub text: heapless::String<96>,
}

pub static SNIPPET_STORE: ::rmk::channel::blocking_mutex::Mutex<
    ::rmk::channel::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<heapless::Vec<SnippetItem, 8>>,
> = ::rmk::channel::blocking_mutex::Mutex::new(core::cell::RefCell::new(heapless::Vec::new()));

pub fn init_default_snippets() {
    SNIPPET_STORE.lock(|cell| {
        let mut v = cell.borrow_mut();
        if v.is_empty() {
            let defaults = [
                ("Git Status", "git status\n"),
                ("Git Diff", "git diff\n"),
                ("Git Commit", "git commit -m \""),
                ("Git Push", "git push\n"),
                ("Cargo Run", "cargo run\n"),
                ("Cargo Build", "cargo build --release\n"),
                ("Curl IP", "curl -s https://ifconfig.me\n"),
                ("Pnpm Dev", "pnpm dev\n"),
            ];
            for (n, t) in defaults {
                let mut item = SnippetItem {
                    name: heapless::String::new(),
                    text: heapless::String::new(),
                };
                let _ = item.name.push_str(n);
                let _ = item.text.push_str(t);
                let _ = v.push(item);
            }
        }
    });
}

pub fn get_snippet_name(idx: usize) -> heapless::String<16> {
    init_default_snippets();
    SNIPPET_STORE.lock(|cell| {
        let v = cell.borrow();
        if let Some(item) = v.get(idx) {
            item.name.clone()
        } else {
            let mut s = heapless::String::new();
            let _ = s.push_str("Snippet");
            s
        }
    })
}

pub fn get_snippet_text(idx: usize) -> heapless::String<96> {
    init_default_snippets();
    SNIPPET_STORE.lock(|cell| {
        let v = cell.borrow();
        if let Some(item) = v.get(idx) {
            item.text.clone()
        } else {
            heapless::String::new()
        }
    })
}

pub fn update_snippet(idx: usize, name: &str, text: &str) {
    init_default_snippets();
    SNIPPET_STORE.lock(|cell| {
        let mut v = cell.borrow_mut();
        if idx < v.len() {
            v[idx].name.clear();
            let _ = v[idx].name.push_str(name);
            v[idx].text.clear();
            let _ = v[idx].text.push_str(text);
        }
    });
}

pub async fn type_snippet_index(idx: usize) {
    let name = get_snippet_name(idx);
    let text = get_snippet_text(idx);
    let mut b: heapless::String<24> = heapless::String::new();
    let _ = core::fmt::write(&mut b, format_args!("OK: {}", name));
    set_alert(&b, 30);
    type_str(&text).await;
}

pub async fn paste_all_snippets() {
    init_default_snippets();
    for idx in 0..8 {
        let text = get_snippet_text(idx);
        if !text.is_empty() {
            type_str(&text).await;
            ::embassy_time::Timer::after(::embassy_time::Duration::from_millis(60)).await;
        }
    }
}
