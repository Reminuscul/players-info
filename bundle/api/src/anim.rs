//! Motion: values that ease toward where they're going, frame by frame, at the same speed
//! whatever the frame rate.

/// `current` moved toward `target`, covering the same share of the way per second whatever
/// `dt`: `rate` is how fast (about 1/rate seconds to get two thirds of the way).
pub fn approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    if !current.is_finite() || !target.is_finite() {
        return target;
    }
    let next = target + (current - target) * (-rate * dt.max(0.0)).exp();
    if (next - target).abs() < 1e-3 {
        target
    } else {
        next
    }
}

/// Fast out, soft landing.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// `argb` with its alpha multiplied by `k`.
pub fn fade(argb: u32, k: f32) -> u32 {
    let alpha = ((argb >> 24) as f32 * k.clamp(0.0, 1.0)).round() as u32;
    (alpha << 24) | (argb & 0x00FF_FFFF)
}

/// An opaque `0xRRGGBB` made `0xAARRGGBB` at alpha `k` (0 to 1).
pub fn opaque(rgb: u32, k: f32) -> u32 {
    fade(0xFF00_0000 | (rgb & 0x00FF_FFFF), k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_is_frame_rate_independent_and_lands() {
        let one = approach(0.0, 1.0, 5.0, 0.1);
        let mut two = approach(0.0, 1.0, 5.0, 0.05);
        two = approach(two, 1.0, 5.0, 0.05);
        assert!((one - two).abs() < 1e-5);
        assert_eq!(approach(0.9995, 1.0, 5.0, 0.016), 1.0);
        assert_eq!(approach(f32::NAN, 2.0, 5.0, 0.1), 2.0);
        assert_eq!(approach(3.0, 1.0, 5.0, 0.0), 3.0);
    }

    #[test]
    fn colours() {
        assert_eq!(fade(0xFF12_3456, 0.5), 0x8012_3456);
        assert_eq!(opaque(0x123456, 1.0), 0xFF12_3456);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(-1.0), 0.0);
    }
}
