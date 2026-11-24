use tuirealm::ratatui::layout::Rect;

/// Split along the visually longer axis (terminal cells are roughly twice as tall as wide).
/// Each half reserves enough cells to keep tiny and empty targets selectable.
pub fn tiles(area: Rect, weights: &[u64], out: &mut Vec<Rect>) {
    if weights.is_empty() || area.is_empty() {
        return;
    }
    if weights.len() == 1 {
        out.push(area);
        return;
    }
    let mid = weights.len() / 2;
    let horizontal = area.width >= area.height.saturating_mul(2) || area.height == 1;
    let horizontal = horizontal && area.width > 1;
    let (length, cross) = if horizontal {
        (area.width, area.height)
    } else {
        (area.height, area.width)
    };
    let sum = |slice: &[u64]| slice.iter().map(|&n| u128::from(n.max(1))).sum::<u128>();
    let left = sum(&weights[..mid]);
    let total = left + sum(&weights[mid..]);
    let min = (mid as u16).div_ceil(cross);
    let max = length - ((weights.len() - mid) as u16).div_ceil(cross);
    // A narrow odd-sized rectangle can fit both groups only along its other axis.
    if min > max {
        for i in 0..weights.len() as u16 {
            out.push(Rect::new(
                area.x + i % area.width,
                area.y + i / area.width,
                1,
                1,
            ));
        }
        return;
    }
    let cut = ((u128::from(length) * left + total / 2) / total) as u16;
    let cut = cut.clamp(min, max);
    let mut first = area;
    let mut second = area;
    if horizontal {
        first.width = cut;
        second.x += cut;
        second.width -= cut;
    } else {
        first.height = cut;
        second.y += cut;
        second.height -= cut;
    }
    tiles(first, &weights[..mid], out);
    tiles(second, &weights[mid..], out);
}
