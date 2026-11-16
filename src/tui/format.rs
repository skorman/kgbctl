//! Number formatting, matching the POC (`poc/kgbpoc.py`).

/// `1234.5` -> `1,234.50` with `decimals` places.
pub fn grouped(x: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, x.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(i, f)| (i, Some(f)));
    let mut out = String::with_capacity(s.len() + int.len() / 3 + 1);
    if x < 0.0 && s.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(frac) = frac {
        out.push('.');
        out.push_str(frac);
    }
    out
}

/// Prices under 1 get four decimals so small crypto prices stay readable.
pub fn price_str(x: f64) -> String {
    grouped(x, if x >= 1.0 { 2 } else { 4 })
}

/// `-$1,234.56`
pub fn money(x: f64) -> String {
    let sign = if x < 0.0 { "-" } else { "" };
    format!("{sign}${}", grouped(x.abs(), 2))
}

/// `+$1,234.56` / `-$1,234.56`
pub fn signed_money(x: f64) -> String {
    if x >= 0.0 { format!("+{}", money(x)) } else { money(x) }
}

/// Fraction as a signed percentage: `0.0523` -> `+5.23%`.
pub fn signed_pct(x: f64) -> String {
    format!("{:+.2}%", x * 100.0)
}

/// Fraction as a percentage: `0.0523` -> `5.2%`.
pub fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(grouped(0.0, 2), "0.00");
        assert_eq!(grouped(999.999, 2), "1,000.00");
        assert_eq!(grouped(1234567.891, 4), "1,234,567.8910");
        assert_eq!(grouped(-1234.5, 2), "-1,234.50");
        assert_eq!(grouped(-0.001, 2), "0.00");
    }

    #[test]
    fn money_and_prices() {
        assert_eq!(money(1234.5), "$1,234.50");
        assert_eq!(money(-1234.5), "-$1,234.50");
        assert_eq!(signed_money(12.0), "+$12.00");
        assert_eq!(signed_money(-12.0), "-$12.00");
        assert_eq!(price_str(1.0), "1.00");
        assert_eq!(price_str(0.12345), "0.1235");
        assert_eq!(price_str(98765.4), "98,765.40");
    }

    #[test]
    fn percentages() {
        assert_eq!(signed_pct(0.0523), "+5.23%");
        assert_eq!(signed_pct(-0.1), "-10.00%");
        assert_eq!(pct(0.256), "25.6%");
    }
}
