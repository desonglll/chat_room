pub(crate) fn like_pattern(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn escapes_sql_wildcards() {
        assert_eq!(like_pattern(r"50%_done\ok"), r"%50\%\_done\\ok%");
    }
}
