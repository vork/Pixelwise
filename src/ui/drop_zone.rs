//! Drag-and-drop loader + file picker.
//!
//! The visible drop zone in the sidebar handles `browse` click; window-level
//! event listeners catch drops anywhere on the page so users don't have to
//! aim for the small sidebar target.

use gloo_events::{EventListener, EventListenerOptions};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, Event, HtmlInputElement};

use crate::io::decode::decode;
use crate::state::store::use_store;

fn load_files_into_store(store: crate::state::store::Store, files: web_sys::FileList, set_busy: WriteSignal<u32>) {
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
}

#[component]
pub fn DropZone() -> impl IntoView {
    let store = use_store();
    let (hover, set_hover) = signal(false);
    let (busy, set_busy) = signal(0_u32);
    let (window_hover, set_window_hover) = signal(false);

    // Window-level drag handlers: accept files dropped anywhere in the page.
    // Without these, dropping outside the small sidebar zone would fall through
    // to the browser default (open the file as a navigation/download).
    Effect::new(move |_| {
        let win = web_sys::window().expect("window");

        // `preventDefault` only works on non-passive listeners, and gloo's
        // default is passive. dragenter/dragover/drop all need preventDefault
        // — without it the browser rejects the drop and downloads the file.
        let active = EventListenerOptions::enable_prevent_default();

        let l_enter = EventListener::new_with_options(&win, "dragenter", active, move |ev| {
            let dev: &DragEvent = ev.unchecked_ref();
            if let Some(dt) = dev.data_transfer() {
                dt.set_drop_effect("copy");
            }
            ev.prevent_default();
            set_window_hover.set(true);
        });
        l_enter.forget();

        let l_over = EventListener::new_with_options(&win, "dragover", active, move |ev| {
            let dev: &DragEvent = ev.unchecked_ref();
            if let Some(dt) = dev.data_transfer() {
                dt.set_drop_effect("copy");
            }
            ev.prevent_default();
        });
        l_over.forget();

        // `relatedTarget == null` on dragleave means the cursor left the page
        // entirely (rather than moving between child elements).
        let l_leave = EventListener::new(&win, "dragleave", move |ev| {
            let dev: &DragEvent = ev.unchecked_ref();
            if dev.related_target().is_none() {
                set_window_hover.set(false);
            }
        });
        l_leave.forget();

        let l_drop = EventListener::new_with_options(&win, "drop", active, move |ev| {
            ev.prevent_default();
            set_window_hover.set(false);
            let dev: &DragEvent = ev.unchecked_ref();
            if let Some(dt) = dev.data_transfer() {
                if let Some(files) = dt.files() {
                    if files.length() > 0 {
                        load_files_into_store(store, files, set_busy);
                    }
                }
            }
        });
        l_drop.forget();
    });

    let on_change = move |ev: Event| {
        let input = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
        if let Some(input) = input {
            if let Some(files) = input.files() {
                load_files_into_store(store, files, set_busy);
            }
            input.set_value("");
        }
    };

    view! {
        <>
            <label
                class="block panel-inset border border-dashed cursor-pointer text-center
                       text-xs text-muted p-3 transition-colors"
                class:border-accentA=move || hover.get()
                on:dragover=move |_| set_hover.set(true)
                on:dragleave=move |_| set_hover.set(false)>
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
            {move || if window_hover.get() {
                view! {
                    <div class="fixed inset-0 z-50 flex items-center justify-center pointer-events-none
                                bg-black/40 backdrop-blur-sm">
                        <div class="panel-inset px-6 py-4 text-base brand-text font-semibold border-2 border-dashed">
                            "Drop images to load"
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <div /> }.into_any()
            }}
        </>
    }
}
