//! Number formatting helpers.

/// Formats `x` with `sig` significant digits, without trailing zeros.
pub fn num(x: f64, sig: usize) -> String {
    if x.is_nan() {
        return String::new();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let sig = sig.max(1);
    let mag = x.abs().log10().floor() as i32;
    if mag >= 9 || mag < -4 {
        let s = format!("{:.*e}", sig - 1, x);
        // Trims mantissa zeros: 1.2300e5 -> 1.23e5
        if let Some((m, e)) = s.split_once('e') {
            let m = if m.contains('.') { m.trim_end_matches('0').trim_end_matches('.') } else { m };
            return format!("{m}e{e}");
        }
        return s;
    }
    let decimals = (sig as i32 - 1 - mag).max(0) as usize;
    let s = format!("{:.*}", decimals, x);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Formats a p-value.
pub fn pval(p: f64) -> String {
    if p.is_nan() {
        String::new()
    } else if p < 0.0001 {
        "< 0.0001".into()
    } else {
        format!("{:.4}", p)
    }
}

/// Formats with a fixed number of decimals.
pub fn fixed(x: f64, d: usize) -> String {
    if x.is_nan() { String::new() } else { format!("{:.*}", d, x) }
}

/// Parses a user-entered number, accepting a decimal comma when no dot is present.
pub fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = t.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    if !t.contains('.') && t.matches(',').count() == 1 {
        if let Ok(v) = t.replace(',', ".").parse::<f64>() {
            return v.is_finite().then_some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats() {
        assert_eq!(super::num(1.23456789, 4), "1.235");
        assert_eq!(super::num(120.0, 4), "120");
        assert_eq!(super::num(0.000012345, 3), "1.23e-5");
        assert_eq!(super::num(-0.5, 3), "-0.5");
        assert_eq!(super::parse_num("1,5"), Some(1.5));
    }
}
