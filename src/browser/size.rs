use tuirealm::ratatui::style::Color;
pub(super) fn gradient(position: f64) -> Color {
    let stops = [
        [110, 150, 249],
        [76, 207, 204],
        [170, 224, 120],
        [255, 203, 91],
        [255, 118, 101],
    ];
    let scaled = position * (stops.len() - 1) as f64;
    let index = (scaled as usize).min(stops.len() - 2);
    let fraction = scaled - index as f64;
    let [red, green, blue] = std::array::from_fn(|i| {
        let from = f64::from(stops[index][i]);
        let to = f64::from(stops[index + 1][i]);
        (from + fraction * (to - from)).round() as u8
    });
    Color::Rgb(red, green, blue)
}

pub(super) fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    for unit in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        value /= 1024.0;
        if value < 1024.0 {
            return format!("{value:.1} {unit}");
        }
    }
    unreachable!("all u64 byte counts fit in EiB")
}

#[derive(Clone, Copy)]
pub(super) struct SizeScale {
    pub smallest: u64,
    pub largest: u64,
}

impl SizeScale {
    pub fn new(sizes: impl Iterator<Item = u64>) -> Self {
        let (smallest, largest) = sizes.fold((u64::MAX, 0), |(lo, hi), n| (lo.min(n), hi.max(n)));
        Self { smallest, largest }
    }

    pub fn color(self, bytes: u64) -> Color {
        let Self { smallest, largest } = self;
        if smallest == largest {
            return gradient(0.5);
        }
        // Expand the lower end so small builds remain distinguishable beside a large outlier.
        gradient(((bytes - smallest) as f64 / (largest - smallest) as f64).sqrt())
    }
}
