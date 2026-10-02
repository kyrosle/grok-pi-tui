/// Human-readable size in powers of 1024: integral bytes (`512 B`), one
/// decimal above (`1.5 MB`). Every output fits nine columns.
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    const UNITS: &[&str] = &["KB", "MB", "GB", "TB", "PB"];
    let mut val = bytes as f64 / 1024.0;
    for unit in UNITS {
        if val < 1023.95 {
            return format!("{val:.1} {unit}");
        }
        val /= 1024.0;
    }
    format!("{val:.1} EB")
}

#[cfg(test)]
mod tests {
    #[test]
    fn format_bytes_preserves_units_and_rounding() {
        for (bytes, expected) in [
            (0, "0 B"),
            (512, "512 B"),
            (1024, "1.0 KB"),
            (1536, "1.5 KB"),
            (1_048_575, "1.0 MB"),
            (1 << 50, "1.0 PB"),
            (u64::MAX, "16.0 EB"),
        ] {
            assert_eq!(super::format_bytes(bytes), expected);
        }
    }
}
