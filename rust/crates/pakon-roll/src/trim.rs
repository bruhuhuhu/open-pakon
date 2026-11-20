//! Sensor-edge inset, applied to a frame in scanner coordinates before the
//! picture is turned upright.
//!
//! The ribbon is `(rows along the film, columns across the film)`. Columns
//! are the sensor: `top` is the high column index, which is the top of the
//! Frames strip and the top of a landscape picture at the export's default
//! quarter turn. `bottom` is column 0. `leading` and `trailing` run along
//! the film (low and high row index, the left and right of a box).
//!
//! The white line on this scanner is a fixed offset of that sensor, not of
//! the film. Flipping the strip leaves it on the same side, and rotating a
//! frame only moves where the line shows up. Trimming here, before
//! [`crate::Orient::apply`], hits that same physical edge on every picture.
//! A spec of all zeros (the default) reproduces the untrimmed crop.

use ndarray::{ArrayView3, Axis};
use serde_json::{json, Value};

/// At most this fraction of a side, per edge, for the optional blank-edge pass.
pub const AUTO_MAX_FRACTION: f64 = 0.02;

/// A blank edge has to be at least this much brighter than the frame's
/// middle, on the positive, so a frame that is bright all over is left alone.
const BLANK_GAP: u16 = 4000;
/// And this uniform: a line that still holds picture detail is not a band.
const BLANK_RANGE: u16 = 12000;

/// Pixels to take off each side. `top` / `bottom` are sensor columns
/// (across the film); `leading` / `trailing` are ribbon rows (along it).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EdgePx {
    pub leading: u32,
    pub trailing: u32,
    pub top: u32,
    pub bottom: u32,
}

impl EdgePx {
    fn any(self) -> bool {
        self.leading + self.trailing + self.top + self.bottom > 0
    }
}

/// Pixels or a percent of that side. Percent of the sensor width (top and
/// bottom) or of the frame length (leading and trailing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimUnit {
    #[default]
    Px,
    Percent,
}

/// What the operator asked for. Default: trim nothing, no auto pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeTrimSpec {
    pub unit: TrimUnit,
    pub leading: f64,
    pub trailing: f64,
    pub top: f64,
    pub bottom: f64,
    /// Also drop a near-uniform bright band, at most [`AUTO_MAX_FRACTION`]
    /// of each side, measured on the frame after the fixed inset.
    pub auto: bool,
}

impl Default for EdgeTrimSpec {
    fn default() -> Self {
        Self {
            unit: TrimUnit::Px,
            leading: 0.0,
            trailing: 0.0,
            top: 0.0,
            bottom: 0.0,
            auto: false,
        }
    }
}

impl EdgeTrimSpec {
    pub fn is_set(self) -> bool {
        self.auto
            || self.leading > 0.0
            || self.trailing > 0.0
            || self.top > 0.0
            || self.bottom > 0.0
    }

    pub fn summary(self) -> String {
        let n = |v: f64| {
            if v.fract() == 0.0 {
                format!("{}", v as i64)
            } else {
                format!("{v:.2}")
            }
        };
        let unit = match self.unit {
            TrimUnit::Px => "px",
            TrimUnit::Percent => "%",
        };
        format!(
            "{unit} leading {}, trailing {}, top {}, bottom {}{}",
            n(self.leading),
            n(self.trailing),
            n(self.top),
            n(self.bottom),
            if self.auto { ", auto" } else { "" }
        )
    }

    /// The settings object (`edge_trim` in settings.json) and the trim the
    /// page sends with an export. Missing or nonsense fields are zero.
    pub fn from_json(v: &Value) -> Self {
        let Some(obj) = v.as_object() else {
            return Self::default();
        };
        let num = |k: &str| -> f64 {
            let n = match obj.get(k) {
                Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
                Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
                _ => 0.0,
            };
            if n.is_finite() && n > 0.0 {
                n
            } else {
                0.0
            }
        };
        let unit = match obj.get("unit").and_then(Value::as_str) {
            Some("percent" | "%") => TrimUnit::Percent,
            _ => TrimUnit::Px,
        };
        let cap = match unit {
            TrimUnit::Percent => 25.0,
            TrimUnit::Px => 10_000.0,
        };
        let edge = |k: &str| num(k).min(cap);
        let auto = match obj.get("auto") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_i64() == Some(1),
            Some(Value::String(s)) => s == "1" || s.eq_ignore_ascii_case("true"),
            _ => false,
        };
        Self {
            unit,
            leading: edge("leading"),
            trailing: edge("trailing"),
            top: edge("top"),
            bottom: edge("bottom"),
            auto,
        }
    }

    pub fn to_json(self) -> Value {
        json!({
            "unit": match self.unit { TrimUnit::Px => "px", TrimUnit::Percent => "percent" },
            "leading": self.leading,
            "trailing": self.trailing,
            "top": self.top,
            "bottom": self.bottom,
            "auto": self.auto,
        })
    }
}

/// `rgb[max(0, r1 - fw):r1]`, `r1 = min(rows, max(0, centre - fw // 2) + fw)`.
pub fn crop_rows(rows: usize, centre: i64, fw: usize) -> (usize, usize) {
    let r1 = ((centre - (fw as i64).div_euclid(2)).max(0) + fw as i64).min(rows as i64);
    ((r1 - fw as i64).max(0) as usize, r1.max(0) as usize)
}

fn edge_px(amount: f64, unit: TrimUnit, side: usize) -> u32 {
    if side == 0 || !amount.is_finite() || amount <= 0.0 {
        return 0;
    }
    let px = match unit {
        TrimUnit::Px => amount,
        TrimUnit::Percent => amount.clamp(0.0, 25.0) / 100.0 * side as f64,
    };
    if !px.is_finite() || px <= 0.0 {
        0
    } else {
        px.round_ties_even().max(0.0) as u32
    }
}

/// Each edge at most a quarter of its side, and the two opposite edges
/// together leave at least half of it.
fn cap_pair(a: u32, b: u32, side: usize) -> (u32, u32) {
    if side == 0 {
        return (0, 0);
    }
    let side = side as u32;
    let mut a = a.min(side / 4);
    let mut b = b.min(side / 4);
    let max_sum = side.saturating_sub((side / 2).max(1));
    let sum = a.saturating_add(b);
    if sum > max_sum && sum > 0 {
        a = (a as u64 * max_sum as u64 / sum as u64) as u32;
        b = (b as u64 * max_sum as u64 / sum as u64) as u32;
    }
    (a, b)
}

/// The fixed inset in scan pixels. `frame_len` is the box along the film,
/// `cols` the sensor width.
pub fn resolve(spec: &EdgeTrimSpec, frame_len: usize, cols: usize) -> EdgePx {
    let (leading, trailing) = cap_pair(
        edge_px(spec.leading, spec.unit, frame_len),
        edge_px(spec.trailing, spec.unit, frame_len),
        frame_len,
    );
    let (bottom, top) = cap_pair(
        edge_px(spec.bottom, spec.unit, cols),
        edge_px(spec.top, spec.unit, cols),
        cols,
    );
    EdgePx {
        leading,
        trailing,
        top,
        bottom,
    }
}

/// How far the auto pass may go: 2% of the frame length along the film and
/// 2% of the sensor width across it, rounded down.
pub fn auto_cap(frame_len: usize, cols: usize) -> EdgePx {
    let cap = |n: usize| (n as f64 * AUTO_MAX_FRACTION).floor() as u32;
    EdgePx {
        leading: cap(frame_len),
        trailing: cap(frame_len),
        top: cap(cols),
        bottom: cap(cols),
    }
}

/// Move `start..end` in by `(lead, trail)`, keeping at least one pixel.
fn shrink_range(start: usize, end: usize, lead: u32, trail: u32) -> (usize, usize) {
    if end <= start {
        return (start, end);
    }
    let len = end - start;
    let a = (lead as usize).min(len.saturating_sub(1));
    let b = (trail as usize).min(len.saturating_sub(a + 1));
    (start + a, end - b)
}

/// `(row0, row1, col0, col1)` of the frame after `trim`, half-open. Columns
/// run across the sensor: `bottom` eats column 0, `top` eats the high end.
pub fn frame_window(
    rows: usize,
    cols: usize,
    centre: i64,
    frame_len: usize,
    trim: EdgePx,
) -> (usize, usize, usize, usize) {
    let (r0, r1) = crop_rows(rows, centre, frame_len);
    let (r0, r1) = shrink_range(r0, r1, trim.leading, trim.trailing);
    let (c0, c1) = shrink_range(0, cols, trim.bottom, trim.top);
    (r0, r1, c0, c1)
}

/// A further inset on a window that is already cropped, same edge order.
pub fn inset(
    r0: usize,
    r1: usize,
    c0: usize,
    c1: usize,
    extra: EdgePx,
) -> (usize, usize, usize, usize) {
    let (r0, r1) = shrink_range(r0, r1, extra.leading, extra.trailing);
    let (c0, c1) = shrink_range(c0, c1, extra.bottom, extra.top);
    (r0, r1, c0, c1)
}

fn brightness_of(sum: u32, n: u32, invert: bool) -> u16 {
    let m = (sum / n.max(1)).min(65535) as u16;
    if invert {
        65535 - m
    } else {
        m
    }
}

fn pixel(crop: ArrayView3<u16>, y: usize, x: usize, invert: bool) -> u16 {
    let c = crop.len_of(Axis(2)).max(1);
    let mut sum = 0u32;
    for k in 0..c {
        sum += crop[(y, x, k)] as u32;
    }
    brightness_of(sum, c as u32, invert)
}

fn line_row(crop: ArrayView3<u16>, y: usize, invert: bool) -> (u16, u16) {
    let w = crop.len_of(Axis(1));
    let mut mn = u16::MAX;
    let mut mx = 0u16;
    for x in 0..w {
        let b = pixel(crop, y, x, invert);
        mn = mn.min(b);
        mx = mx.max(b);
    }
    (mn, mx)
}

fn line_col(crop: ArrayView3<u16>, x: usize, invert: bool) -> (u16, u16) {
    let h = crop.len_of(Axis(0));
    let mut mn = u16::MAX;
    let mut mx = 0u16;
    for y in 0..h {
        let b = pixel(crop, y, x, invert);
        mn = mn.min(b);
        mx = mx.max(b);
    }
    (mn, mx)
}

fn percentile(samples: &mut [u16], p: f64) -> u16 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let i = ((samples.len() - 1) as f64 * p).round_ties_even() as usize;
    samples[i.min(samples.len() - 1)]
}

fn is_blank(min_b: u16, max_b: u16, p98: u16, median: u16) -> bool {
    min_b >= p98
        && min_b >= median.saturating_add(BLANK_GAP)
        && max_b.saturating_sub(min_b) <= BLANK_RANGE
}

fn blank_run(
    len: usize,
    max_n: usize,
    mut stats: impl FnMut(usize) -> (u16, u16),
    p98: u16,
    median: u16,
) -> u32 {
    let max_n = max_n.min(len.saturating_sub(1));
    let mut n = 0usize;
    while n < max_n {
        let (mn, mx) = stats(n);
        if !is_blank(mn, mx, p98, median) {
            break;
        }
        n += 1;
    }
    n as u32
}

/// Pixels of near-uniform bright band on each edge of `crop`. `invert` is
/// for a negative: low raw becomes bright on the positive. A slide passes
/// `false` (high raw is already bright). Nothing is returned once an edge
/// stops being blank, and never more than `max_px` on that edge.
pub fn detect_blank_edges(crop: ArrayView3<u16>, invert: bool, max_px: EdgePx) -> EdgePx {
    let (h, w, _) = crop.dim();
    if h < 2 || w < 2 || !max_px.any() {
        return EdgePx::default();
    }
    let (sy, sx) = ((h / 64).max(1), (w / 64).max(1));
    let mut samples = Vec::with_capacity((h / sy) * (w / sx));
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            samples.push(pixel(crop, y, x, invert));
            x += sx;
        }
        y += sy;
    }
    if samples.len() < 16 {
        return EdgePx::default();
    }
    let median = percentile(&mut samples, 0.5);
    let p98 = percentile(&mut samples, 0.98);
    EdgePx {
        leading: blank_run(
            h,
            max_px.leading as usize,
            |d| line_row(crop, d, invert),
            p98,
            median,
        ),
        trailing: blank_run(
            h,
            max_px.trailing as usize,
            |d| line_row(crop, h - 1 - d, invert),
            p98,
            median,
        ),
        bottom: blank_run(
            w,
            max_px.bottom as usize,
            |d| line_col(crop, d, invert),
            p98,
            median,
        ),
        top: blank_run(
            w,
            max_px.top as usize,
            |d| line_col(crop, w - 1 - d, invert),
            p98,
            median,
        ),
    }
}

/// One window per centre: the fixed inset, then the blank-edge pass when
/// `spec.auto` is set. `width_of(i)` is that frame's length in ribbon rows.
pub fn windows(
    rgb: ArrayView3<u16>,
    centres: &[i64],
    width_of: impl Fn(usize) -> usize,
    spec: &EdgeTrimSpec,
    invert: bool,
) -> Vec<(usize, usize, usize, usize)> {
    let rows = rgb.len_of(Axis(0));
    let cols = rgb.len_of(Axis(1));
    centres
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let fw = width_of(i);
            let fixed = resolve(spec, fw, cols);
            let (r0, r1, c0, c1) = frame_window(rows, cols, c, fw, fixed);
            if !spec.auto {
                return (r0, r1, c0, c1);
            }
            let extra = detect_blank_edges(
                rgb.slice(ndarray::s![r0..r1, c0..c1, ..]),
                invert,
                auto_cap(fw, cols),
            );
            inset(r0, r1, c0, c1, extra)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array3;

    fn spec_px(top: f64, bottom: f64, leading: f64, trailing: f64) -> EdgeTrimSpec {
        EdgeTrimSpec {
            unit: TrimUnit::Px,
            top,
            bottom,
            leading,
            trailing,
            auto: false,
        }
    }

    #[test]
    fn zero_trim_is_the_full_crop() {
        assert_eq!(crop_rows(10000, 5000, 3000), (3500, 6500));
        assert_eq!(crop_rows(10000, 100, 3000), (0, 3000));
        assert_eq!(crop_rows(10000, 9900, 3000), (7000, 10000));
        let z = EdgePx::default();
        assert_eq!(
            frame_window(10000, 1500, 5000, 3000, z),
            (3500, 6500, 0, 1500)
        );
        assert!(!EdgeTrimSpec::default().is_set());
    }

    #[test]
    fn top_is_the_high_sensor_column_and_bottom_is_column_zero() {
        let px = resolve(&spec_px(10.0, 4.0, 6.0, 8.0), 2250, 1500);
        assert_eq!(
            px,
            EdgePx {
                leading: 6,
                trailing: 8,
                top: 10,
                bottom: 4
            }
        );
        // centre 4000, length 2250 → rows 2875..5125, then leading/trailing.
        assert_eq!(
            frame_window(20000, 1500, 4000, 2250, px),
            (2881, 5117, 4, 1490)
        );
    }

    #[test]
    fn percent_is_of_the_sensor_width_across_and_the_frame_along() {
        let spec = EdgeTrimSpec {
            unit: TrimUnit::Percent,
            top: 0.5,
            bottom: 0.0,
            leading: 1.0,
            trailing: 0.0,
            auto: false,
        };
        // 0.5% of 1500 = 7.5 → 8; 1% of 2250 = 22.5 → 22 (half to even).
        let px = resolve(&spec, 2250, 1500);
        assert_eq!(px.top, 8);
        assert_eq!(px.leading, 22);
        assert_eq!(px.bottom, 0);
    }

    #[test]
    fn an_edge_stops_at_a_quarter_of_its_side() {
        let px = resolve(&spec_px(10_000.0, 10_000.0, 0.0, 0.0), 2250, 1500);
        assert!(px.top + px.bottom <= 1500 / 2);
        assert!(px.top <= 1500 / 4 && px.bottom <= 1500 / 4);
        let hung = frame_window(
            100,
            40,
            5,
            50,
            EdgePx {
                leading: 10,
                ..EdgePx::default()
            },
        );
        // the box hangs off the start (rows 0..50); leading still applies.
        assert_eq!(hung, (10, 50, 0, 40));
    }

    #[test]
    fn json_round_trip_clamps_nonsense() {
        let raw =
            json!({"unit": "percent", "top": 80, "bottom": -3, "leading": "1.5", "auto": true});
        let spec = EdgeTrimSpec::from_json(&raw);
        assert_eq!(spec.unit, TrimUnit::Percent);
        assert_eq!(spec.top, 25.0);
        assert_eq!(spec.bottom, 0.0);
        assert_eq!(spec.leading, 1.5);
        assert!(spec.auto);
        assert_eq!(EdgeTrimSpec::from_json(&spec.to_json()), spec);
        assert_eq!(
            EdgeTrimSpec::from_json(&Value::Null),
            EdgeTrimSpec::default()
        );
        assert_eq!(
            EdgeTrimSpec::from_json(&json!({"unit": "nope", "top": 10})).unit,
            TrimUnit::Px
        );
    }

    fn paint(h: usize, w: usize, fill: u16) -> Array3<u16> {
        Array3::from_elem((h, w, 3), fill)
    }

    #[test]
    fn auto_trims_only_a_uniform_bright_sensor_edge_and_caps_at_two_percent() {
        // Negative: the overrun is dark on the raw and white on the positive.
        let mut a = paint(40, 200, 28000);
        for y in 0..40 {
            for x in 197..200 {
                for c in 0..3 {
                    a[(y, x, c)] = 150;
                }
            }
        }
        let cap = auto_cap(40, 200);
        assert_eq!(cap.top, 4);
        let found = detect_blank_edges(a.view(), true, cap);
        assert_eq!(
            found,
            EdgePx {
                top: 3,
                ..EdgePx::default()
            }
        );

        // Ten columns of band, but 2% of 100 is 2.
        let mut wide = paint(30, 100, 28000);
        for y in 0..30 {
            for x in 90..100 {
                for c in 0..3 {
                    wide[(y, x, c)] = 150;
                }
            }
        }
        let found = detect_blank_edges(wide.view(), true, auto_cap(30, 100));
        assert_eq!(found.top, 2);
        assert_eq!(found.bottom, 0);

        // A detailed edge is not a band, and neither is a frame that is all white.
        let mut noisy = paint(30, 80, 28000);
        for y in 0..30 {
            noisy[(y, 79, 0)] = if y % 2 == 0 { 150 } else { 28000 };
            noisy[(y, 79, 1)] = noisy[(y, 79, 0)];
            noisy[(y, 79, 2)] = noisy[(y, 79, 0)];
        }
        assert_eq!(
            detect_blank_edges(noisy.view(), true, auto_cap(30, 80)),
            EdgePx::default()
        );
        assert_eq!(
            detect_blank_edges(paint(30, 80, 150).view(), true, auto_cap(30, 80)),
            EdgePx::default()
        );
    }

    #[test]
    fn auto_on_a_slide_looks_for_high_raw_and_stacks_on_the_fixed_inset() {
        let mut slide = paint(32, 160, 8000);
        for y in 0..32 {
            for c in 0..3 {
                slide[(y, 0, c)] = 64000;
                slide[(y, 1, c)] = 64000;
            }
        }
        let found = detect_blank_edges(slide.view(), false, auto_cap(32, 160));
        assert_eq!(found.bottom, 2);
        assert_eq!(found.top, 0);

        let mut neg = paint(36, 150, 28000);
        for y in 0..36 {
            for x in 145..150 {
                for c in 0..3 {
                    neg[(y, x, c)] = 100;
                }
            }
        }
        let spec = EdgeTrimSpec {
            top: 2.0,
            auto: true,
            ..spec_px(0.0, 0.0, 0.0, 0.0)
        };
        let w = windows(neg.view(), &[18], |_| 36, &spec, true);
        // fixed 2 columns, then the 3 that are still blank. 2% of 150 is 3.
        assert_eq!(w, vec![(0, 36, 0, 145)]);
    }

    #[test]
    fn fixed_trim_that_already_clears_the_band_adds_nothing() {
        let mut neg = paint(36, 150, 28000);
        for y in 0..36 {
            for x in 147..150 {
                for c in 0..3 {
                    neg[(y, x, c)] = 100;
                }
            }
        }
        let spec = EdgeTrimSpec {
            top: 3.0,
            auto: true,
            ..EdgeTrimSpec::default()
        };
        let w = windows(neg.view(), &[18], |_| 36, &spec, true);
        assert_eq!(w, vec![(0, 36, 0, 147)]);
    }
}
