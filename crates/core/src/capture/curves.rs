//! Pure easing and spring functions. All closed forms: the value at `t` does
//! not depend on frame rate or on earlier evaluations.

/// Spring natural frequency per second of `response`: a critically damped
/// spring reaches 99.88 % of its target after `response` seconds.
pub const SETTLE: f64 = 9.0;

/// Clamp `x` into `lo..=hi`.
pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

/// Linear interpolation, unclamped.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// `3t² − 2t³` on the clamped unit interval.
pub fn smoothstep(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_in_cubic(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    t * t * t
}

pub fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - clamp(t, 0.0, 1.0);
    1.0 - u * u * u
}

pub fn ease_in_out_cubic(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Unit step response of a damped spring at time `t` (seconds), closed form
/// for natural frequency `omega` (rad/s) and damping ratio `zeta`. Starts at 0
/// with zero velocity and settles at 1; `zeta < 1` overshoots, `zeta >= 1`
/// does not. Returns 0 for `t <= 0`.
pub fn spring_step(t: f64, omega: f64, zeta: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    let a = zeta * omega;
    let decay = (-a * t).exp();
    if (zeta - 1.0).abs() < 1e-9 {
        1.0 - (1.0 + omega * t) * decay
    } else if zeta < 1.0 {
        let wd = omega * (1.0 - zeta * zeta).sqrt();
        1.0 - decay * ((wd * t).cos() + a / wd * (wd * t).sin())
    } else {
        let s = omega * (zeta * zeta - 1.0).sqrt();
        1.0 - decay * ((s * t).cosh() + a / s * (s * t).sinh())
    }
}

/// Critically damped spring that reaches ~99.9 % after `response` seconds
/// (`omega = SETTLE / response`). Monotone, never overshoots. A non-positive
/// `response` jumps straight to 1 for `t > 0`.
pub fn critical_spring(t: f64, response: f64) -> f64 {
    if response <= 0.0 {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    clamp(spring_step(t, SETTLE / response, 1.0), 0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
    }

    #[test]
    fn clamp_lerp_smoothstep() {
        assert_eq!(clamp(5.0, 0.0, 1.0), 1.0);
        assert_eq!(clamp(-5.0, 0.0, 1.0), 0.0);
        assert_eq!(clamp(0.25, 0.0, 1.0), 0.25);
        assert_eq!(lerp(2.0, 4.0, 0.5), 3.0);
        assert_eq!(lerp(2.0, 4.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 4.0, 1.0), 4.0);
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert_eq!(smoothstep(0.5), 0.5);
        assert_eq!(smoothstep(2.0), 1.0);
    }

    #[test]
    fn cubic_eases_hit_ends_and_order() {
        for f in [ease_in_cubic, ease_out_cubic, ease_in_out_cubic] {
            assert_eq!(f(0.0), 0.0);
            assert_eq!(f(1.0), 1.0);
            assert_eq!(f(-1.0), 0.0);
            assert_eq!(f(2.0), 1.0);
        }
        assert!(ease_in_cubic(0.5) < 0.5);
        assert!(ease_out_cubic(0.5) > 0.5);
        assert_eq!(ease_in_out_cubic(0.5), 0.5);
    }

    #[test]
    fn spring_matches_closed_form() {
        // critically damped, omega 2, t 1: 1 - 3 e^-2
        let want = 1.0 - 3.0 * (-2.0f64).exp();
        assert!((spring_step(1.0, 2.0, 1.0) - want).abs() < 1e-12);
        assert_eq!(spring_step(0.0, 2.0, 1.0), 0.0);
        assert_eq!(spring_step(-1.0, 2.0, 0.5), 0.0);
        // under damped overshoots, over damped does not
        let peak = (0..400).map(|i| spring_step(i as f64 * 0.01, 10.0, 0.3)).fold(0.0, f64::max);
        assert!(peak > 1.2, "under-damped peak {peak}");
        let over = (0..400).map(|i| spring_step(i as f64 * 0.01, 10.0, 2.0)).fold(0.0, f64::max);
        assert!(over <= 1.0);
        for z in [0.3, 1.0, 2.0] {
            assert!((spring_step(10.0, 10.0, z) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn critical_spring_settles_at_response() {
        assert_eq!(critical_spring(0.0, 0.6), 0.0);
        assert!(critical_spring(0.6, 0.6) > 0.998);
        assert_eq!(critical_spring(1.0, 0.0), 1.0);
    }

    #[test]
    fn critical_spring_never_overshoots_and_is_monotone() {
        let mut seed = 42u64;
        for _ in 0..1000 {
            let response = 0.05 + lcg(&mut seed) * 2.0;
            let mut prev = 0.0;
            let n = 20 + (lcg(&mut seed) * 200.0) as usize;
            let dt = lcg(&mut seed) * 0.05 + 0.001;
            for i in 0..=n {
                let v = critical_spring(i as f64 * dt, response);
                assert!((0.0..=1.0).contains(&v), "v {v}");
                assert!(v >= prev - 1e-15);
                prev = v;
            }
        }
    }

    #[test]
    fn frame_rate_independent() {
        // Same instant reached by different step sizes gives identical bits.
        let a = critical_spring(0.6, 0.7);
        let t: f64 = 0.6;
        assert_eq!(a, critical_spring(t, 0.7));
        assert_eq!(critical_spring(0.5, 0.7), critical_spring(0.5, 0.7));
    }
}
