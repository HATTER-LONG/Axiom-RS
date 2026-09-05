//! Strongly typed calculations for the reference host.

use axiom_rs::BusinessFailure;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Summary {
    pub count: i64,
    pub min: i64,
    pub max: i64,
    pub mean: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BoxMeasure {
    pub volume: f64,
    pub surface: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dilution {
    pub aliquot_ml: f64,
    pub diluent_ml: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TempScale {
    C,
    F,
    K,
}

/// Summarize integer samples. Empty input is a business failure.
pub(crate) fn summarize_samples(samples: &[i64]) -> Result<Summary, BusinessFailure> {
    if samples.is_empty() {
        return Err(BusinessFailure::new("samples must not be empty"));
    }
    let count = i64::try_from(samples.len())
        .map_err(|_| BusinessFailure::new("sample count does not fit an integer"))?;
    let min = samples.iter().copied().min().expect("non-empty");
    let max = samples.iter().copied().max().expect("non-empty");
    let sum: i128 = samples.iter().copied().map(i128::from).sum();
    let mean = finite((sum as f64) / (count as f64), "mean")?;
    Ok(Summary {
        count,
        min,
        max,
        mean,
    })
}

/// Axis-aligned box volume and surface from edge lengths.
pub(crate) fn axis_aligned_box(x: f64, y: f64, z: f64) -> Result<BoxMeasure, BusinessFailure> {
    let volume = product(&[x, y, z], "volume")?;
    let xy = product(&[x, y], "surface")?;
    let yz = product(&[y, z], "surface")?;
    let zx = product(&[z, x], "surface")?;
    let surface = finite(2.0 * finite(xy + yz + zx, "surface")?, "surface")?;
    Ok(BoxMeasure { volume, surface })
}

/// Dilution volumes. `target > stock` is a business failure.
pub(crate) fn dilute(
    stock_mm: f64,
    target_mm: f64,
    volume_ml: f64,
) -> Result<Dilution, BusinessFailure> {
    if target_mm > stock_mm {
        return Err(BusinessFailure::new("target exceeds stock concentration"));
    }
    let aliquot_ml = mul_div(volume_ml, target_mm, stock_mm, "aliquot_mL")?;
    let diluent_ml = finite(volume_ml - aliquot_ml, "diluent_mL")?;
    Ok(Dilution {
        aliquot_ml,
        diluent_ml,
    })
}

/// Convert a temperature between Celsius, Fahrenheit, and Kelvin.
pub(crate) fn convert_temperature(
    value: f64,
    from: TempScale,
    to: TempScale,
) -> Result<f64, BusinessFailure> {
    let kelvin = finite(to_kelvin(value, from), "temperature")?;
    if kelvin < 0.0 {
        return Err(BusinessFailure::new("temperature is below absolute zero"));
    }
    finite(from_kelvin(kelvin, to), "temperature")
}

fn to_kelvin(value: f64, unit: TempScale) -> f64 {
    match unit {
        TempScale::K => value,
        TempScale::C => value + 273.15,
        TempScale::F => (value - 32.0) * (5.0 / 9.0) + 273.15,
    }
}

fn from_kelvin(kelvin: f64, unit: TempScale) -> f64 {
    match unit {
        TempScale::K => kelvin,
        TempScale::C => kelvin - 273.15,
        TempScale::F => (kelvin - 273.15) * (9.0 / 5.0) + 32.0,
    }
}

fn product(factors: &[f64], what: &str) -> Result<f64, BusinessFailure> {
    combine(factors, &[], what)
}

fn mul_div(a: f64, b: f64, c: f64, what: &str) -> Result<f64, BusinessFailure> {
    combine(&[a, b], &[c], what)
}

fn combine(factors: &[f64], divisors: &[f64], what: &str) -> Result<f64, BusinessFailure> {
    if factors
        .iter()
        .chain(divisors)
        .any(|value| !value.is_finite())
        || divisors.contains(&0.0)
    {
        return Err(not_finite(what));
    }
    if factors.contains(&0.0) {
        return Ok(0.0);
    }
    let mut sign = 1.0;
    let mut mantissa = 1.0;
    let mut exp = 0;
    for value in factors {
        sign *= value.signum();
        let (part, part_exp) = split_abs(value.abs());
        mantissa *= part;
        exp += part_exp;
        let (normalized, extra) = split_abs(mantissa);
        mantissa = normalized;
        exp += extra;
    }
    for value in divisors {
        sign *= value.signum();
        let (part, part_exp) = split_abs(value.abs());
        mantissa /= part;
        exp -= part_exp;
        let (normalized, extra) = split_abs(mantissa);
        mantissa = normalized;
        exp += extra;
    }
    finite(sign * scale_pow2(mantissa, exp), what)
}

fn split_abs(value: f64) -> (f64, i32) {
    if value == 0.0 {
        return (0.0, 0);
    }
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    if biased == 0 {
        let mut scaled = value;
        let mut exp = 0;
        while scaled < 0.5 {
            scaled *= 2.0;
            exp -= 1;
        }
        return (scaled, exp);
    }
    let exp = biased - 1023;
    let fraction = bits & ((1_u64 << 52) - 1);
    let mantissa = f64::from_bits(fraction | (0x3ff_u64 << 52));
    (mantissa / 2.0, exp + 1)
}

fn scale_pow2(mantissa: f64, exp: i32) -> f64 {
    if mantissa == 0.0 {
        return 0.0;
    }
    let bits = mantissa.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1_u64 << 52) - 1);
    let (mut significand, unbiased) = if biased == 0 {
        (frac, -1022)
    } else {
        (frac | (1_u64 << 52), biased - 1023)
    };
    let Some(mut unbiased) = unbiased.checked_add(exp) else {
        return if exp > 0 { f64::INFINITY } else { 0.0 };
    };
    if significand == 0 {
        return 0.0;
    }
    if significand < (1_u64 << 52) {
        let align = significand.leading_zeros() as i32 - 11;
        significand <<= align;
        unbiased -= align;
    }
    if unbiased > 1023 {
        return f64::INFINITY;
    }
    if unbiased >= -1022 {
        let new_biased = (unbiased + 1023) as u64;
        return f64::from_bits((new_biased << 52) | (significand & ((1_u64 << 52) - 1)));
    }
    let Some(shift) = (-1022i32).checked_sub(unbiased) else {
        return 0.0;
    };
    round_to_subnormal(significand, shift)
}

fn round_to_subnormal(significand: u64, shift: i32) -> f64 {
    if shift <= 0 {
        return f64::from_bits(1_u64 << 52);
    }
    if shift >= 64 {
        return 0.0;
    }
    let shift = shift as u32;
    let mut frac = significand >> shift;
    let round_bit = (significand >> (shift - 1)) & 1;
    let sticky = shift > 1 && significand & ((1_u64 << (shift - 1)) - 1) != 0;
    if round_bit == 1 && (sticky || frac & 1 == 1) {
        frac += 1;
    }
    if frac >= (1_u64 << 52) {
        return f64::from_bits(1_u64 << 52);
    }
    f64::from_bits(frac)
}

fn not_finite(what: &str) -> BusinessFailure {
    BusinessFailure::new(format!("{what} cannot be represented as a finite number"))
}

fn finite(value: f64, what: &str) -> Result<f64, BusinessFailure> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(BusinessFailure::new(format!(
            "{what} cannot be represented as a finite number"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_extremes_and_mixed_signs() {
        let max = summarize_samples(&[i64::MAX, i64::MAX]).unwrap();
        assert_eq!(max.count, 2);
        assert_eq!(max.min, i64::MAX);
        assert!((max.mean - (i64::MAX as f64)).abs() < 1e9);
        let min = summarize_samples(&[i64::MIN, i64::MIN]).unwrap();
        assert_eq!(min.max, i64::MIN);
        let mixed = summarize_samples(&[i64::MIN, -1, 0, 1, i64::MAX]).unwrap();
        assert_eq!(mixed.min, i64::MIN);
        assert_eq!(mixed.max, i64::MAX);
        let typical = summarize_samples(&[2, 4]).unwrap();
        assert!((typical.mean - 3.0).abs() < f64::EPSILON);
        assert!(summarize_samples(&[]).is_err());
    }

    fn relative_close(actual: f64, expected: f64) -> bool {
        if !actual.is_finite() || !expected.is_finite() {
            return false;
        }
        if actual == 0.0 && expected == 0.0 {
            return true;
        }
        if actual == 0.0 || expected == 0.0 {
            return false;
        }
        (actual - expected).abs() / expected.abs() < 1e-10
    }

    fn assert_rel(actual: f64, expected: f64) {
        assert!(
            relative_close(actual, expected),
            "actual={actual} expected={expected}"
        );
    }

    #[test]
    fn relative_close_rejects_known_bad_tiny_results() {
        assert!(relative_close(3e-24, 3e-24));
        assert!(!relative_close(4.940_656_458_412_465e-24, 3e-24));
        assert!(!relative_close(0.0, 1e-300));
        assert!(!relative_close(0.0, 1e-200));
        assert!(!relative_close(4.940_656_458_412_465e-124, 3e-124));
    }

    #[test]
    fn box_and_dilute_numeric_boundaries() {
        let ok = axis_aligned_box(2.0, 3.0, 4.0).unwrap();
        assert!((ok.volume - 24.0).abs() < f64::EPSILON);
        assert!(axis_aligned_box(1e200, 1e200, 1.0).is_err());
        let recovered = axis_aligned_box(1e-200, 1e-200, 1e200).unwrap();
        assert_rel(recovered.volume, 1e-200);
        let subnormal_mid = axis_aligned_box(1e-200, 3e-124, 1e200).unwrap();
        assert_rel(subnormal_mid.volume, 3e-124);
        let tiny = axis_aligned_box(1e-200, 2.0, 3.0).unwrap();
        assert_rel(tiny.volume, 6e-200);
        assert!(dilute(1.0, 2.0, 10.0).is_err());
        let mix = dilute(1e200, 1e-200, 1e-200).unwrap();
        assert!(mix.aliquot_ml.is_finite());
        let under = dilute(1e300, 1e-300, 1e300).unwrap();
        assert_rel(under.aliquot_ml, 1e-300);
        let lost_bits = dilute(1e300, 3e-24, 1e300).unwrap();
        assert_rel(lost_bits.aliquot_ml, 3e-24);
        let reordered = dilute(1e20, 1e20, 1e300).unwrap();
        assert_rel(reordered.aliquot_ml, 1e300);
        assert!(reordered.diluent_ml.is_finite());
        let min_pos = f64::from_bits(1);
        let max_subnormal = f64::from_bits((1_u64 << 52) - 1);
        let min_normal = f64::from_bits(1_u64 << 52);
        for target in [min_pos, max_subnormal, min_normal] {
            let round_trip = dilute(1.0, target, 1.0).unwrap();
            assert_eq!(round_trip.aliquot_ml, target);
        }
        let subnormal_target = dilute(1.0, 1e-310, 1e300).unwrap();
        assert_rel(subnormal_target.aliquot_ml, 1e-10);
        let pow2 = f64::from_bits(((1023 - 537) as u64) << 52);
        let min_volume = axis_aligned_box(pow2, pow2, 1.0).unwrap();
        assert_eq!(min_volume.volume, min_pos);
        let half = f64::from_bits(0x3fe0_0000_0000_0000);
        let below = f64::from_bits(half.to_bits() - 1);
        let above = f64::from_bits(half.to_bits() + 1);
        assert_eq!(
            axis_aligned_box(pow2, pow2, below)
                .unwrap()
                .volume
                .to_bits(),
            0
        );
        assert_eq!(
            axis_aligned_box(pow2, pow2, half).unwrap().volume.to_bits(),
            0
        );
        assert_eq!(
            axis_aligned_box(pow2, pow2, above)
                .unwrap()
                .volume
                .to_bits(),
            min_pos.to_bits()
        );
    }

    #[test]
    fn temperature_boundaries() {
        assert!(
            (convert_temperature(0.0, TempScale::C, TempScale::K).unwrap() - 273.15).abs() < 1e-9
        );
        assert!(convert_temperature(-274.0, TempScale::C, TempScale::K).is_err());
        let from_f = convert_temperature(1e308, TempScale::F, TempScale::K).unwrap();
        assert_rel(from_f, (1e308 - 32.0) * (5.0 / 9.0) + 273.15);
        let back = convert_temperature(from_f, TempScale::K, TempScale::F).unwrap();
        assert_rel(back, 1e308);
        assert!(convert_temperature(f64::MAX, TempScale::C, TempScale::F).is_err());
        assert!(convert_temperature(f64::MAX, TempScale::K, TempScale::F).is_err());
        assert!(
            convert_temperature(1e-300, TempScale::K, TempScale::C)
                .unwrap()
                .is_finite()
        );
    }
}
