use ferephono::file_name_formatter;

#[test]
fn formats_pascal_case_file_reference() {
    assert_eq!(
        file_name_formatter::format_spoken_references("the recording-implementation.java"),
        "the @RecordingImplementation.java"
    );
}

#[test]
fn formats_kebab_case_file_reference() {
    assert_eq!(
        file_name_formatter::format_spoken_references("the MD recording schedule.yaml"),
        "the @md-recording-schedule.yaml"
    );
    assert_eq!(
        file_name_formatter::format_spoken_references("the project management.md"),
        "the @project-management.md"
    );
}

#[test]
fn formats_file_reference_inside_prose() {
    assert_eq!(
        file_name_formatter::format_spoken_references(
            "Please update the developer dictionary.swift file"
        ),
        "Please update the @DeveloperDictionary.swift file"
    );
}

#[test]
fn supports_spoken_dot_and_extension_aliases() {
    assert_eq!(
        file_name_formatter::format_spoken_references("Open the transcript processor dot rust."),
        "Open the @transcript_processor.rs."
    );
}

#[test]
fn leaves_normal_prose_unchanged() {
    let prose = "Please update developer dictionary.swift after lunch.";
    assert_eq!(file_name_formatter::format_spoken_references(prose), prose);
}

#[test]
fn preserves_surrounding_punctuation() {
    assert_eq!(
        file_name_formatter::format_spoken_references("Can you inspect the project management.md?"),
        "Can you inspect the @project-management.md?"
    );
}

#[test]
fn rejects_identifiers_longer_than_five_words() {
    let transcript = "the one two three four five six.java";
    assert_eq!(
        file_name_formatter::format_spoken_references(transcript),
        transcript
    );
}

#[test]
fn processes_multiple_file_references() {
    assert_eq!(
        file_name_formatter::format_spoken_references(
            "Compare the first model.java with the second model.kt"
        ),
        "Compare the @FirstModel.java with the @SecondModel.kt"
    );
}

#[test]
fn supports_spelled_out_extensions() {
    for (spoken, expected) in [
        ("r s", "rs"),
        ("p y", "py"),
        ("j s", "js"),
        ("t s", "ts"),
        ("j s x", "jsx"),
        ("t s x", "tsx"),
    ] {
        assert_eq!(
            file_name_formatter::format_spoken_references(&format!("Open the file name dot {spoken}.")),
            format!("Open the @file_name.{expected}.")
        );
    }
}

#[test]
fn processes_multiple_references_with_spelled_out_extensions() {
    assert_eq!(
        file_name_formatter::format_spoken_references(
            "Would the file name front matter dot r s and the file name for matter dot r s be the same?"
        ),
        "Would the @file_name_front_matter.rs and the @file_name_for_matter.rs be the same?"
    );
}

#[test]
fn does_not_consume_prose_after_spelled_out_extension() {
    assert_eq!(
        file_name_formatter::format_spoken_references("Open the file name dot r s I mentioned."),
        "Open the @file_name.rs I mentioned."
    );
    let unsupported = "Open the file name dot r z.";
    assert_eq!(file_name_formatter::format_spoken_references(unsupported), unsupported);
}
