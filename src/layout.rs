//! Area-preserving squarified treemap; geometry is independent of rendering.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Bounds {
    pub fn area(self) -> f64 {
        f64::from(self.w) * f64::from(self.h)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub id: usize,
    pub rect: Bounds,
}
fn worst(row: &[(usize, f64)], side: f64) -> f64 {
    let sum: f64 = row.iter().map(|x| x.1).sum();
    if sum <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let max = row.iter().map(|x| x.1).fold(0.0, f64::max);
    let min = row.iter().map(|x| x.1).fold(f64::INFINITY, f64::min);
    (side * side * max / (sum * sum)).max(sum * sum / (side * side * min))
}
fn place(row: &[(usize, f64)], space: &mut Bounds, result: &mut Vec<Tile>) {
    let sum: f64 = row.iter().map(|x| x.1).sum();
    let vertical = space.w >= space.h;
    let side = if vertical { space.h } else { space.w };
    if side <= 0.0 {
        return;
    }
    let strip = (sum / f64::from(side)) as f32;
    let mut offset = 0.0;
    for (index, &(id, area)) in row.iter().enumerate() {
        let length = if index + 1 == row.len() {
            side - offset
        } else {
            (area / f64::from(strip.max(f32::MIN_POSITIVE))) as f32
        };
        let rect = if vertical {
            Bounds {
                x: space.x,
                y: space.y + offset,
                w: strip,
                h: length,
            }
        } else {
            Bounds {
                x: space.x + offset,
                y: space.y,
                w: length,
                h: strip,
            }
        };
        result.push(Tile { id, rect });
        offset += length;
    }
    if vertical {
        space.x += strip;
        space.w = (space.w - strip).max(0.0);
    } else {
        space.y += strip;
        space.h = (space.h - strip).max(0.0);
    }
}
pub fn treemap(weights: &[(usize, u64)], mut space: Bounds) -> Vec<Tile> {
    let total: f64 = weights.iter().map(|x| x.1 as f64).sum();
    if total == 0.0 || space.area() <= 0.0 {
        return Vec::new();
    }
    let scale = space.area() / total;
    let mut values: Vec<_> = weights
        .iter()
        .filter(|x| x.1 > 0)
        .map(|&(id, size)| (id, size as f64 * scale))
        .collect();
    values.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut result = Vec::with_capacity(values.len());
    let mut start = 0;
    while start < values.len() {
        let side = f64::from(space.w.min(space.h));
        let mut end = start + 1;
        while end < values.len()
            && worst(&values[start..=end], side) <= worst(&values[start..end], side)
        {
            end += 1;
        }
        place(&values[start..end], &mut space, &mut result);
        start = end;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_area_and_proportions_without_overlap() {
        let weights: Vec<_> = (0..257).map(|i| (i, (i + 1) as u64)).collect();
        let bounds = Bounds {
            x: 20.0,
            y: 30.0,
            w: 1000.0,
            h: 600.0,
        };
        let tiles = treemap(&weights, bounds);
        let total: u64 = weights.iter().map(|x| x.1).sum();
        assert_eq!(tiles.len(), weights.len());
        for (i, t) in tiles.iter().enumerate() {
            let expected = bounds.area() * weights[t.id].1 as f64 / total as f64;
            assert!((t.rect.area() - expected).abs() < 0.5);
            assert!(t.rect.x >= bounds.x - 0.01 && t.rect.y >= bounds.y - 0.01);
            assert!(t.rect.x + t.rect.w <= bounds.x + bounds.w + 0.01);
            assert!(t.rect.y + t.rect.h <= bounds.y + bounds.h + 0.01);
            for other in &tiles[i + 1..] {
                let overlap_w = (t.rect.x + t.rect.w).min(other.rect.x + other.rect.w)
                    - t.rect.x.max(other.rect.x);
                let overlap_h = (t.rect.y + t.rect.h).min(other.rect.y + other.rect.h)
                    - t.rect.y.max(other.rect.y);
                assert!(overlap_w <= 0.01 || overlap_h <= 0.01);
            }
        }
    }
    #[test]
    fn empty_zero_and_extreme_weights_are_finite() {
        let b = Bounds {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        assert!(treemap(&[], b).is_empty());
        assert!(treemap(&[(1, 0)], b).is_empty());
        for t in treemap(&[(1, u64::MAX), (2, 1)], b) {
            assert!(t.rect.area().is_finite());
        }
    }
}
