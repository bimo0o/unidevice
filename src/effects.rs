#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Gradient,
    Breathing,
    Rainbow,
    Timeline,
}

impl Pattern {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gradient => "Градиент",
            Self::Breathing => "Дыхание",
            Self::Rainbow => "Радуга",
            Self::Timeline => "Конструктор",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyframe {
    pub at: f32,
    pub color: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    Linear,
    #[default]
    Smooth,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub pattern: Pattern,
    pub first: [u8; 3],
    pub second: [u8; 3],
    pub seconds: f32,
    #[serde(default)]
    pub easing: Easing,
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}
impl Default for Effect {
    fn default() -> Self {
        Self {
            pattern: Pattern::Gradient,
            first: [83, 129, 255],
            second: [242, 153, 117],
            seconds: 8.0,
            easing: Easing::Smooth,
            keyframes: vec![
                Keyframe {
                    at: 0.0,
                    color: [83, 129, 255],
                },
                Keyframe {
                    at: 0.5,
                    color: [242, 153, 117],
                },
            ],
        }
    }
}
impl Effect {
    pub fn valid(&self) -> bool {
        self.seconds.is_finite()
            && (4.0..=30.0).contains(&self.seconds)
            && self.keyframes.len() <= 8
            && (self.pattern != Pattern::Timeline
                || (self.keyframes.len() >= 2 && self.keyframes[0].at == 0.0))
            && self
                .keyframes
                .iter()
                .all(|k| k.at.is_finite() && (0.0..=0.98).contains(&k.at))
            && self.keyframes.windows(2).all(|w| w[1].at - w[0].at >= 0.02)
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
            Pattern::Timeline => {
                if !self.valid() {
                    return self.first;
                }
                let index = self
                    .keyframes
                    .iter()
                    .rposition(|k| f64::from(k.at) <= phase)
                    .unwrap_or(0);
                let a = &self.keyframes[index];
                let (end, color) = self
                    .keyframes
                    .get(index + 1)
                    .map_or((1.0, self.keyframes[0].color), |b| {
                        (f64::from(b.at), b.color)
                    });
                let mut t = (phase - f64::from(a.at)) / (end - f64::from(a.at));
                if self.easing == Easing::Smooth {
                    t = (1.0 - (t * std::f64::consts::PI).cos()) * 0.5;
                }
                blend(a.color, color, t)
            }
        }
    }
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    #[test]
    fn cyclic_timeline_is_continuous_and_rejects_hostile_points() {
        let mut e = Effect {
            pattern: Pattern::Timeline,
            ..Effect::default()
        };
        assert!(e.valid());
        assert_eq!(
            e.color(std::time::Duration::from_secs(4)),
            e.keyframes[1].color
        );
        assert_eq!(
            e.color(std::time::Duration::from_secs(8)),
            e.keyframes[0].color
        );
        assert_eq!(
            e.color(std::time::Duration::from_millis(7999)),
            e.keyframes[0].color
        );
        e.keyframes[1].at = 0.0;
        assert!(!e.valid());
        e.keyframes[1].at = f32::NAN;
        assert!(!e.valid());
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
            ..Default::default()
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
            ..Default::default()
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
