//! Human-readable byte sizes and counts. Sizes use binary units
//! (1 KiB = 1024 B).

const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
/// 2^64: the first value that no longer fits in a `u64`.
const MAX_BYTES: f64 = 18_446_744_073_709_551_616.0;

/// Renders a byte count such as `1.5 MiB`.
#[must_use]
pub fn human_size(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    #[allow(clippy::cast_precision_loss)] // display only
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Error returned by [`parse_size`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid size {0:?}")]
pub struct ParseSizeError(String);

/// Parses a human size such as `512`, `10K`, `1.5MiB` or `2 GB`. Units are
/// binary (1K = 1024) whatever the spelling.
///
/// # Errors
/// [`ParseSizeError`] on an empty number, a negative value, an unknown unit
/// or a value that does not fit in `u64`.
pub fn parse_size(s: &str) -> Result<u64, ParseSizeError> {
    let lower = s.trim().to_ascii_lowercase();
    let split = lower
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(lower.len());
    let (num, unit) = lower.split_at(split);
    let mult = multiplier(unit.trim()).ok_or_else(|| ParseSizeError(s.to_owned()))?;
    let value: f64 = num.parse().map_err(|_| ParseSizeError(s.to_owned()))?;
    #[allow(clippy::cast_precision_loss)] // bounds check only
    let bytes = value * mult as f64;
    if num.is_empty() || value < 0.0 || !bytes.is_finite() || bytes >= MAX_BYTES {
        return Err(ParseSizeError(s.to_owned()));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked above
    Ok(bytes as u64)
}

fn multiplier(unit: &str) -> Option<u64> {
    Some(match unit {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1 << 10,
        "m" | "mb" | "mib" => 1 << 20,
        "g" | "gb" | "gib" => 1 << 30,
        "t" | "tb" | "tib" => 1 << 40,
        _ => return None,
    })
}

/// Renders a count with a plain ASCII space between each group of three
/// digits, counted from the right: `48210` renders as `48 210`.
#[must_use]
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        let left = digits.len() - i;
        if i > 0 && left.is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
