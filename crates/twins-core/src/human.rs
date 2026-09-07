//! Human-readable byte sizes, binary units (1 KiB = 1024 B).

const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

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
/// [`ParseSizeError`] on an empty number, a negative value or an unknown unit.
pub fn parse_size(s: &str) -> Result<u64, ParseSizeError> {
    let lower = s.trim().to_ascii_lowercase();
    let split = lower
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(lower.len());
    let (num, unit) = lower.split_at(split);
    let mult = multiplier(unit.trim()).ok_or_else(|| ParseSizeError(s.to_owned()))?;
    let value: f64 = num.parse().map_err(|_| ParseSizeError(s.to_owned()))?;
    if num.is_empty() || value < 0.0 || !value.is_finite() {
        return Err(ParseSizeError(s.to_owned()));
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    Ok((value * mult as f64) as u64)
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
