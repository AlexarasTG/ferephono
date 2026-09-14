#[derive(Clone, Copy)]
pub(super) enum IdentifierStyle {
    Pascal,
    Camel,
    Snake,
    Kebab,
}

pub(super) fn canonical_extension(token: &str) -> Option<&'static str> {
    match token
        .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
        .to_lowercase()
        .as_str()
    {
        "java" => Some("java"),
        "kt" | "kotlin" => Some("kt"),
        "swift" => Some("swift"),
        "py" | "python" => Some("py"),
        "rs" | "rust" => Some("rs"),
        "json" => Some("json"),
        "log" => Some("log"),
        "yaml" => Some("yaml"),
        "yml" => Some("yml"),
        "xml" => Some("xml"),
        "md" | "markdown" => Some("md"),
        "js" | "javascript" => Some("js"),
        "ts" | "typescript" => Some("ts"),
        "jsx" => Some("jsx"),
        "tsx" => Some("tsx"),
        _ => None,
    }
}

pub(super) fn identifier_style(extension: &str) -> IdentifierStyle {
    match extension {
        "java" | "kt" | "swift" => IdentifierStyle::Pascal,
        "py" | "rs" | "js" | "ts" | "jsx" | "tsx" => IdentifierStyle::Snake,
        "json" | "yaml" | "yml" | "xml" | "log" | "md" => IdentifierStyle::Kebab,
        _ => IdentifierStyle::Camel,
    }
}

pub(super) fn is_supported_extension(extension: &str) -> bool {
    canonical_extension(extension).is_some_and(|canonical| canonical == extension)
}
