use tuirealm::ratatui::layout::Rect;

pub const TILE_HEIGHT: u16 = 4;

// The caller limits the page to the number of label-sized rectangles that fit.
pub fn tiles(area: Rect, weights: &[u64], label_width: u16, out: &mut Vec<Rect>) {
    if weights.len() == 1 {
        out.push(area);
        return;
    }
    let columns = area.width / label_width;
    let rows = area.height / TILE_HEIGHT;
    let horizontal = rows == 1 || (columns > 1 && area.width >= area.height.saturating_mul(2));
    let (length, cross_slots, unit) = if horizontal {
        (area.width, rows, label_width)
    } else {
        (area.height, columns, TILE_HEIGHT)
    };
    let slots = length / unit;
    let count = weights.len() as u16;
    let mut mid = count / 2;
    // When both halves would need a partially filled strip, move the split to
    // a strip boundary so neither child loses its last label slot.
    if mid.div_ceil(cross_slots) + (count - mid).div_ceil(cross_slots) > slots {
        mid = mid.div_ceil(cross_slots) * cross_slots;
    }
    let min = mid.div_ceil(cross_slots) * unit;
    let max = length - (count - mid).div_ceil(cross_slots) * unit;
    let (first_weights, second_weights) = weights.split_at(usize::from(mid));
    let sum = |slice: &[u64]| slice.iter().map(|&n| u128::from(n.max(1))).sum::<u128>();
    let left = sum(first_weights);
    let total = left + sum(second_weights);
    let cut = (((u128::from(length) * left + total / 2) / total) as u16).clamp(min, max);
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
    tiles(first, first_weights, label_width, out);
    tiles(second, second_weights, label_width, out);
}
