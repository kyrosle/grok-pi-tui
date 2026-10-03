pub use xai_tool_types::presentation::format_bytes;

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
