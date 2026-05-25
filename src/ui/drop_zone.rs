//! Drag-and-drop loader + file picker.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, Event, HtmlInputElement};

use crate::io::decode::decode;
use crate::state::store::use_store;

#[component]
pub fn DropZone() -> impl IntoView {
    let store = use_store();
    let (hover, set_hover) = signal(false);
    let (busy, set_busy) = signal(0_u32);

    let load_files = move |files: web_sys::FileList| {
        for i in 0..files.length() {
            let Some(file) = files.item(i) else { continue };
            let name = file.name();
            set_busy.update(|n| *n += 1);
            spawn_local(async move {
                let bytes = match gloo_file::futures::read_as_bytes(&gloo_file::Blob::from(file))
                    .await
                {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("read {name}: {e:?}");
                        set_busy.update(|n| *n = n.saturating_sub(1));
                        return;
                    }
                };
                match decode(bytes, &name) {
                    Ok(img) => {
                        log::info!("loaded {} ({}×{} {})", name, img.width, img.height, img.format_label);
                        store.add_image(img);
                    }
                    Err(e) => log::error!("decode {name}: {e}"),
                }
                set_busy.update(|n| *n = n.saturating_sub(1));
            });
        }
    };

    let on_drop = {
        let load_files = load_files.clone();
        move |ev: DragEvent| {
            ev.prevent_default();
            set_hover.set(false);
            if let Some(dt) = ev.data_transfer() {
                if let Some(files) = dt.files() {
                    load_files(files);
                }
            }
        }
    };

    let on_change = {
        let load_files = load_files.clone();
        move |ev: Event| {
            let input = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
            if let Some(input) = input {
                if let Some(files) = input.files() {
                    load_files(files);
                }
                input.set_value("");
            }
        }
    };

    view! {
        <label
            class="block panel-inset border border-dashed cursor-pointer text-center
                   text-xs text-muted p-3 transition-colors"
            class:border-accentA=move || hover.get()
            on:dragover=move |ev: DragEvent| { ev.prevent_default(); set_hover.set(true); }
            on:dragleave=move |_| set_hover.set(false)
            on:drop=on_drop>
            {move || if busy.get() == 0 {
                view! {
                    <div>
                        <span class="brand-text font-medium">"Drop images"</span>
                        " or "
                        <span class="underline">"browse"</span>
                        <div class="text-[10px] mt-1">"EXR · HDR · PNG · TIFF · JPEG"</div>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="brand-text font-medium">
                        {format!("Decoding {} file(s)…", busy.get())}
                    </div>
                }.into_any()
            }}
            <input
                type="file"
                multiple=true
                accept=".exr,.hdr,.pic,.png,.tif,.tiff,.jpg,.jpeg,image/*"
                class="hidden"
                on:change=on_change
            />
        </label>
    }
}
