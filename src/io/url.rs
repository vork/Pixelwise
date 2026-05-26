//! Fetch images by URL and add them to the store. Used by drag-drop's
//! sister path: shareable links / embeds via `?a=&b=` query parameters.

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::io::decode::decode;
use crate::state::store::Store;

/// Try to fetch `url`, decode the bytes as a known image format, and add the
/// result to the store. Returns immediately if fetch or decode fails (logged
/// to console). Used for sharable `?a=URL&b=URL` query parameters; the
/// remote host must serve the file with permissive CORS or the fetch fails.
pub async fn fetch_and_load(store: Store, url: String) {
    let win = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let resp_val = match JsFuture::from(win.fetch_with_str(&url)).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("fetch {url}: {e:?}");
            return;
        }
    };
    let resp: Response = match resp_val.dyn_into() {
        Ok(r) => r,
        Err(_) => {
            log::error!("fetch {url}: bad response type");
            return;
        }
    };
    if !resp.ok() {
        log::error!("fetch {url}: HTTP {}", resp.status());
        return;
    }
    let buf_promise = match resp.array_buffer() {
        Ok(p) => p,
        Err(e) => {
            log::error!("fetch {url}: array_buffer: {e:?}");
            return;
        }
    };
    let buf = match JsFuture::from(buf_promise).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("fetch {url}: read: {e:?}");
            return;
        }
    };
    let arr = js_sys::Uint8Array::new(&buf);
    let bytes = arr.to_vec();
    let name = file_name_from_url(&url);
    match decode(bytes, &name) {
        Ok(img) => {
            log::info!("loaded {} from URL ({}×{} {})", name, img.width, img.height, img.format_label);
            store.add_image(img);
        }
        Err(e) => log::error!("decode {name} from {url}: {e}"),
    }
}

fn file_name_from_url(url: &str) -> String {
    let path = url.split(&['?', '#'][..]).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("remote-image")
        .to_string()
}

/// Read `?a=&b=&img=` from the current page URL and queue fetches for each.
/// `a`/`b` go to primary/secondary explicitly (in that order); repeated `img`
/// entries are added via the normal auto-assignment in `Store::add_image`.
pub fn load_from_query(store: Store) {
    let win = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let search = win.location().search().unwrap_or_default();
    if search.is_empty() {
        return;
    }
    let params = match web_sys::UrlSearchParams::new_with_str(&search) {
        Ok(p) => p,
        Err(_) => return,
    };

    // Collect URLs in stable order: a, b, then every `img=`.
    let mut targets: Vec<String> = Vec::new();
    if let Some(a) = params.get("a") {
        if !a.is_empty() {
            targets.push(a);
        }
    }
    if let Some(b) = params.get("b") {
        if !b.is_empty() {
            targets.push(b);
        }
    }
    let img_iter = params.get_all("img");
    for i in 0..img_iter.length() {
        if let Some(s) = img_iter.get(i).as_string() {
            if !s.is_empty() {
                targets.push(s);
            }
        }
    }

    if targets.is_empty() {
        return;
    }
    log::info!("loading {} image(s) from URL parameters", targets.len());
    for url in targets {
        let store = store;
        leptos::task::spawn_local(async move {
            fetch_and_load(store, url).await;
        });
    }
}
