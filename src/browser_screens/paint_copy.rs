//! One paint's pixels into a screen's block, keeping only what really changed.
//! Chromium reports where it drew, not whether the pixels differ.

/// Runs of changed rows closer than this go up as one rectangle.
const MERGE_ROWS: u32 = 16;
const CHUNK: usize = 64;

fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let mut at = 0;
    for (x, y) in a.chunks(CHUNK).zip(b.chunks(CHUNK)) {
        if x != y {
            return x.iter().zip(y).position(|(p, q)| p != q).map(|i| at + i);
        }
        at += x.len();
    }
    None
}

fn last_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let mut end = a.len();
    for (x, y) in a.rchunks(CHUNK).zip(b.rchunks(CHUNK)) {
        if x != y {
            return x.iter().zip(y).rposition(|(p, q)| p != q).map(|i| end - x.len() + i);
        }
        end -= x.len();
    }
    None
}

/// `rect` is x, y, w, h in pixels, inside the frame. At most `max_rects` go on `out`.
pub fn copy_changed(src: &[u8], dst: &mut [u8], stride: usize, rect: [u32; 4], max_rects: usize, out: &mut Vec<[u32; 4]>) {
    let [x, y, w, h] = rect;
    let (from, len) = (x as usize * 4, w as usize * 4);
    let limit = out.len() + max_rects;
    // x0, y0, x1, y1
    let mut span: Option<[u32; 4]> = None;
    for row in y..y + h {
        let at = row as usize * stride + from;
        let (Some(a), Some(b)) = (src.get(at..at + len), dst.get_mut(at..at + len)) else { continue };
        let Some(first) = first_diff(a, b) else { continue };
        let last = last_diff(a, b).unwrap_or(first);
        let (p0, p1) = (first / 4, last / 4 + 1);
        b[p0 * 4..p1 * 4].copy_from_slice(&a[p0 * 4..p1 * 4]);
        let (x0, x1) = (x + p0 as u32, x + p1 as u32);
        span = Some(match span {
            Some(s) if row - s[3] < MERGE_ROWS || out.len() + 1 >= limit => [s[0].min(x0), s[1], s[2].max(x1), row + 1],
            Some(s) => {
                out.push([s[0], s[1], s[2] - s[0], s[3] - s[1]]);
                [x0, row, x1, row + 1]
            }
            None => [x0, row, x1, row + 1],
        });
    }
    if let Some(s) = span {
        if max_rects > 0 {
            out.push([s[0], s[1], s[2] - s[0], s[3] - s[1]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 16;
    const H: usize = 200;

    fn frame(fill: u8) -> Vec<u8> {
        vec![fill; W * H * 4]
    }

    fn set(frame: &mut [u8], x: usize, y: usize, v: u8) {
        frame[(y * W + x) * 4] = v;
    }

    #[test]
    fn an_identical_rectangle_reports_nothing_and_leaves_the_block_alone() {
        let (src, mut dst) = (frame(5), frame(5));
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn one_changed_pixel_reports_one_pixel() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 3, 7, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert_eq!(out, vec![[3, 7, 1, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn a_change_in_the_last_byte_of_a_row_is_found() {
        let (mut src, mut dst) = (frame(5), frame(5));
        src[W * 4 * 3 - 1] = 9;
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert_eq!(out, vec![[W as u32 - 1, 2, 1, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn wide_rows_past_a_chunk_are_trimmed_on_both_sides() {
        const WIDE: usize = 100;
        let (mut src, mut dst) = (vec![5u8; WIDE * 4 * 4], vec![5u8; WIDE * 4 * 4]);
        src[(WIDE + 30) * 4] = 9;
        src[(WIDE + 80) * 4 + 2] = 9;
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, WIDE * 4, [0, 0, WIDE as u32, 4], 32, &mut out);
        assert_eq!(out, vec![[30, 1, 51, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn close_rows_merge_and_far_rows_stay_apart() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 1, 10, 9);
        set(&mut src, 2, 12, 9);
        set(&mut src, 8, 100, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 32, &mut out);
        assert_eq!(out, vec![[1, 10, 2, 3], [8, 100, 1, 1]]);
        assert_eq!(dst, src);
    }

    #[test]
    fn never_more_rectangles_than_allowed() {
        let (mut src, mut dst) = (frame(5), frame(5));
        for y in (0..H).step_by(40) {
            set(&mut src, 0, y, 9);
        }
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 0, W as u32, H as u32], 2, &mut out);
        assert!(out.len() <= 2);
        assert_eq!(dst, src);
        for y in (0..H).step_by(40) {
            assert!(out.iter().any(|r| r[0] == 0 && (y as u32) >= r[1] && (y as u32) < r[1] + r[3]));
        }
    }

    #[test]
    fn only_the_given_rectangle_is_looked_at() {
        let (mut src, mut dst) = (frame(5), frame(5));
        set(&mut src, 3, 7, 9);
        let mut out = Vec::new();
        copy_changed(&src, &mut dst, W * 4, [0, 20, W as u32, 10], 32, &mut out);
        assert!(out.is_empty());
        assert_eq!(dst, frame(5));
    }
}
