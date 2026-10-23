#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Gradient,
    Breathing,
    Rainbow,
}

impl Pattern {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gradient => "Градиент",
            Self::Breathing => "Дыхание",
            Self::Rainbow => "Радуга",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub pattern: Pattern,
    pub first: [u8; 3],
    pub second: [u8; 3],
    pub seconds: f32,
}
impl Effect {
    pub fn valid(&self) -> bool {
        self.seconds.is_finite() && (4.0..=30.0).contains(&self.seconds)
    }
    pub fn color(&self, elapsed: std::time::Duration) -> [u8; 3] {
        let phase =
            (elapsed.as_secs_f64() / f64::from(self.seconds.clamp(4.0, 30.0))).rem_euclid(1.0);
        let mix = (1.0 - (phase * std::f64::consts::TAU).cos()) / 2.0;
        match self.pattern {
            Pattern::Gradient => blend(self.first, self.second, mix),
            Pattern::Breathing => blend(self.first, [0; 3], mix * 0.95),
            Pattern::Rainbow => {
                let colors = [
                    [255, 0, 0],
                    [255, 255, 0],
                    [0, 255, 0],
                    [0, 255, 255],
                    [0, 0, 255],
                    [255, 0, 255],
                    [255, 0, 0],
                ];
                let segment = phase * 6.0;
                let index = segment.floor() as usize;
                blend(colors[index], colors[index + 1], segment.fract())
            }
        }
    }
}
fn blend(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    std::array::from_fn(|i| (f64::from(a[i]) * (1.0 - t) + f64::from(b[i]) * t).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gradient_endpoints_and_period_are_deterministic() {
        let e = Effect {
            pattern: Pattern::Gradient,
            first: [255, 0, 0],
            second: [0, 0, 255],
            seconds: 8.0,
        };
        assert_eq!(e.color(std::time::Duration::ZERO), e.first);
        assert_eq!(e.color(std::time::Duration::from_secs(4)), e.second);
        assert_eq!(e.color(std::time::Duration::from_secs(8)), e.first);
        assert_eq!(e.color(std::time::Duration::from_secs(2)), [128, 0, 127]);
    }
    #[test]
    fn invalid_speed_is_rejected_and_rainbow_wraps() {
        let mut e = Effect {
            pattern: Pattern::Rainbow,
            first: [0; 3],
            second: [0; 3],
            seconds: 6.0,
        };
        assert!(e.valid());
        assert_eq!(e.color(std::time::Duration::from_secs(1)), [255, 255, 0]);
        assert_eq!(e.color(std::time::Duration::from_secs(6)), [255, 0, 0]);
        e.seconds = f32::NAN;
        assert!(!e.valid());
        e.seconds = 0.0;
        assert!(!e.valid());
    }
}
