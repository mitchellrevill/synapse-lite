//! Recovered Studio effect palette geometry and mask presentation.
use super::*;
use secbird_core::studio::document::NewEffect;

const EFFECTS: [(&str, &str, NewEffect); 13] = [
    ("Ambient\nAwareness", "ambient", NewEffect::Ambient),
    ("Audio\nMeter", "audio", NewEffect::Audio),
    ("Breathing", "breathing", NewEffect::Breathing),
    ("Chroma\nGenerate", "chroma-generate", NewEffect::ChromaGenerate),
    ("Fire", "fire", NewEffect::Fire),
    ("Reactive", "reactive", NewEffect::Reactive),
    ("Ripple", "ripple", NewEffect::Ripple),
    ("Spectrum\nCycling", "spectrum", NewEffect::Spectrum),
    ("Starlight", "starlight", NewEffect::Starlight),
    ("Static", "static", NewEffect::Static),
    ("Tidal", "tidal", NewEffect::Tidal),
    ("Wave", "wave", NewEffect::Wave),
    ("Wheel", "wheel", NewEffect::Wheel),
];

pub(super) fn palette_top(height: i32) -> i32 { (height - 410).max(208) }
pub(super) fn max_scroll(height: i32) -> i32 {
    (360 - (height - palette_top(height) - 40)).max(0)
}

pub(super) unsafe fn hover(state: &mut State, message: u32, lparam: isize) {
    if !state.studio.open || !matches!(message, WM_MOUSEMOVE | WM_MOUSELEAVE) { return; }
    let point = (message != WM_MOUSELEAVE).then(|| (
        ((lparam as u32 & 0xffff) as i16 as f32 / state.scale) as i32,
        (((lparam as u32 >> 16) & 0xffff) as i16 as f32 / state.scale) as i32,
    )).filter(|(x,y)| (0..250).contains(x) && *y>=128);
    if state.studio.workspace_hover != point {
        state.studio.workspace_hover = point;
        // Only the left panel depends on this pointer state.
        let rect = RECT { left: 0, top: (128. * state.scale) as i32,
            right: (250. * state.scale) as i32, bottom: state.height };
        InvalidateRect(state.hwnd, &rect, 0);
    }
}

pub(super) unsafe fn icon(state: &State, p: &Painter, role: &str, tint: &str,
    x: i32, y: i32, size: i32) {
    p.image(state.icons.get(format!("studio-{role}-{tint}").as_str()), x, y, size, size);
}

pub(super) unsafe fn paint_palette(state: &State, p: &mut Painter, height: i32) {
    let top = palette_top(height);
    p.fill(0, top, 250, height-top, BG);
    p.fill(0, top, 250, 1, 0x383838);
    p.text("ADD EFFECT", 10, top, 230, 40, p.fonts.small, 0x707070, DT_LEFT);
    let clip = SaveDC(p.dc);
    IntersectClipRect(p.dc, 0, p.px(top+40), p.px(250), p.px(height));
    let enabled = state.studio.editable() && !state.read_only_smoke && !state.restricted_actions;
    for (index, (title, role, effect)) in EFFECTS.iter().enumerate() {
        let x = 11 + (index % 3) as i32 * 76;
        let y = top + 40 + (index / 3) as i32 * 72 - state.studio.palette_scroll;
        if y+72 <= top+40 || y >= height { continue; }
        let hover = state.studio.workspace_hover.is_some_and(|(px,py)|
            px>=x && px<x+76 && py>=y.max(top+40) && py<(y+72).min(height));
        if hover && enabled { p.fill(x,y,76,72,0x181818); }
        icon(state,p,role,if hover && enabled {"normal"} else {"muted"},x+23,y+8,30);
        for (line, text) in title.split('\n').enumerate() {
            p.text(text,x+2,y+40+line as i32*12,72,12,p.fonts.tiny,
                if hover && enabled {TEXT} else {0x707070},DT_CENTER);
        }
        if enabled { p.hits.push(Hit { rect:p.rect(x,y.max(top+40),76,(y+72).min(height)-y.max(top+40)),
            action:Action::Studio(super::Control::Layers(layer_tools::Control::Add(*effect))) }); }
    }
    RestoreDC(p.dc,clip);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_reserves_five_rows_and_scrolls_on_small_windows() {
        assert_eq!(palette_top(1104),694);
        assert_eq!(max_scroll(1104),0);
        assert_eq!(palette_top(600),208);
        assert_eq!(max_scroll(600),8);
        assert_eq!(EFFECTS.len(),13);
        for (index, (_, role, effect)) in EFFECTS.iter().enumerate() {
            assert!(EFFECTS[..index].iter().all(|(_,other_role,other)|role!=other_role && effect!=other));
        }
    }
}
