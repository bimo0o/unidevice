use crate::error::{Error, Result};
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feel {
    pub measured_cm360: f64,
    pub reference_dpi: u16,
    pub game_sensitivity: f64,
}
impl Feel {
    pub fn valid(&self) -> bool {
        self.measured_cm360.is_finite()
            && (1.0..=300.0).contains(&self.measured_cm360)
            && (100..=25600).contains(&self.reference_dpi)
            && self.game_sensitivity.is_finite()
            && (0.0001..=1000.0).contains(&self.game_sensitivity)
    }
    pub fn sensitivity_at(&self, dpi: u16) -> Result<f64> {
        if !self.valid() || !(100..=25600).contains(&dpi) {
            return Err(Error::Invalid("sensitivity reference".into()));
        }
        Ok(self.game_sensitivity * f64::from(self.reference_dpi) / f64::from(dpi))
    }
    pub fn dpi_for_distance(&self, wanted_cm: f64) -> Result<f64> {
        if !self.valid() || !wanted_cm.is_finite() || !(1.0..=300.0).contains(&wanted_cm) {
            return Err(Error::Invalid("cm/360 reference".into()));
        }
        Ok(f64::from(self.reference_dpi) * self.measured_cm360 / wanted_cm)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sensitivity_preserves_same_game_scale_and_distance() {
        let f = Feel {
            measured_cm360: 40.0,
            reference_dpi: 800,
            game_sensitivity: 2.0,
        };
        assert_eq!(f.sensitivity_at(1600).unwrap(), 1.0);
        assert_eq!(f.dpi_for_distance(20.0).unwrap(), 1600.0);
        assert!(f.dpi_for_distance(f64::NAN).is_err());
        assert!(f.sensitivity_at(0).is_err());
    }
}
