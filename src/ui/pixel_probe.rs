//! Overlay readout in the bottom-left of the viewport, showing raw +
//! display-referred values at the cursor, plus NaN/Inf/negative flags.

use leptos::prelude::*;

use crate::color::tonemap;
use crate::state::store::use_store;
use crate::state::view::Tonemap;

#[derive(Clone)]
struct ProbeRead {
    x: i32,
    y: i32,
    raw: [f32; 4],
    exposed: [f32; 3],
    display: [f32; 3],
    flags: Vec<String>,
    secondary: Option<[f32; 4]>,
}

#[component]
pub fn PixelProbe() -> impl IntoView {
    let store = use_store();
    let read = Signal::derive(move || -> Option<ProbeRead> {
        let (x, y) = store.probe_px.get()?;
        let img = store.primary_image()?;
        let raw = img.pixel(x as u32, y as u32)?;
        let mul = 2f32.powf(store.exposure.get());
        let exposed = [raw[0] * mul, raw[1] * mul, raw[2] * mul];
        let display = match store.tonemap.get() {
            Tonemap::Linear => tonemap::linear(exposed),
            Tonemap::Reinhard => tonemap::reinhard(exposed),
            Tonemap::Aces => tonemap::aces_fitted(exposed),
            Tonemap::Filmic => tonemap::filmic_hejl(exposed),
            Tonemap::Gamma => tonemap::gamma(exposed, 2.2),
        };
        let mut flags = Vec::new();
        for (n, v) in ["R", "G", "B", "A"].iter().zip(raw.iter()) {
            if v.is_nan() {
                flags.push(format!("NaN({n})"));
            } else if v.is_infinite() {
                flags.push(format!("Inf({n})"));
            } else if *v < 0.0 {
                flags.push(format!("Neg({n})"));
            }
        }
        let secondary = store.secondary_image().and_then(|img2| img2.pixel(x as u32, y as u32));
        Some(ProbeRead { x, y, raw, exposed, display, flags, secondary })
    });

    view! {
        {move || read.get().map(|p| {
            view! {
                <div class="absolute left-2 bottom-2 panel-inset px-3 py-2 text-[11px] nums shadow-soft pointer-events-none">
                    <div class="flex items-center gap-2 mb-1">
                        <span class="label">"Probe"</span>
                        <span class="text-text">{format!("({}, {})", p.x, p.y)}</span>
                        {p.flags.clone().into_iter().map(|f| view! { <span class="chip-err">{f}</span> }).collect_view()}
                    </div>
                    <ProbeTable p=p />
                </div>
            }
        })}
    }
}

#[component]
fn ProbeTable(p: ProbeRead) -> impl IntoView {
    let has_b = p.secondary.is_some();
    let rows: Vec<_> = ["R", "G", "B"].iter().enumerate().map(|(i, n)| {
        let raw_v = p.raw[i];
        let exp_v = p.exposed[i];
        let disp_v = p.display[i];
        let sec = p.secondary;
        view! {
            <tr>
                <td class="pr-3">{*n}</td>
                <td class="pr-3 text-right">{format_val(raw_v)}</td>
                <td class="pr-3 text-right">{format_val(exp_v)}</td>
                <td class="text-right">{format_val(disp_v)}</td>
                {match sec {
                    Some(s) => {
                        let bv = s[i];
                        let dv = bv - raw_v;
                        let cls = if dv.abs() > 1e-3 { "pl-2 text-right text-err" } else { "pl-2 text-right text-muted" };
                        view! {
                            <td class="pl-3 text-right">{format_val(bv)}</td>
                            <td class=cls>{format_val(dv)}</td>
                        }.into_any()
                    }
                    None => view! { <td /> }.into_any(),
                }}
            </tr>
        }
    }).collect();
    let alpha = p.raw[3];

    view! {
        <table class="text-[11px]">
            <thead class="text-muted">
                <tr>
                    <th class="pr-3 text-left">"chan"</th>
                    <th class="pr-3 text-right">"raw"</th>
                    <th class="pr-3 text-right">"× EV"</th>
                    <th class="text-right">"display"</th>
                    {if has_b {
                        view! {
                            <th class="pl-3 text-right">"B"</th>
                            <th class="pl-2 text-right">"Δ"</th>
                        }.into_any()
                    } else {
                        view! { <th /> }.into_any()
                    }}
                </tr>
            </thead>
            <tbody>
                {rows}
                <tr>
                    <td class="pr-3">"A"</td>
                    <td class="pr-3 text-right">{format_val(alpha)}</td>
                    <td />
                    <td />
                </tr>
            </tbody>
        </table>
    }
}

fn format_val(v: f32) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() {
        if v > 0.0 { "+Inf".into() } else { "-Inf".into() }
    } else if v.abs() >= 100.0 {
        format!("{:.1}", v)
    } else if v.abs() >= 1.0 {
        format!("{:.3}", v)
    } else {
        format!("{:.4}", v)
    }
}
