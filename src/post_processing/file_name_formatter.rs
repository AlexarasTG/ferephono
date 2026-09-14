use super::supported_file_types::{IdentifierStyle, canonical_extension, identifier_style};
use std::ops::Range;

const MAXIMUM_IDENTIFIER_WORD_COUNT: usize = 5;

pub fn format_spoken_references(transcript: &str) -> String {
    format_with_resolver(transcript, |name| name.to_owned())
}

pub(super) fn format_with_resolver(
    transcript: &str,
    mut resolve: impl FnMut(&str) -> String,
) -> String {
    let tokens = tokenize(transcript);
    let mut matches = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if tokens[index].normalized != "the" {
            index += 1;
            continue;
        }

        if let Some(code_match) = parse_phrase(&tokens, index) {
            index = code_match.last_token_index + 1;
            matches.push(code_match);
        } else {
            index += 1;
        }
    }

    let mut processed = transcript.to_owned();
    for code_match in matches.into_iter().rev() {
        let identifier = format_identifier(
            &code_match.identifier_words,
            identifier_style(&code_match.extension),
        );
        let name = format!("{identifier}.{}", code_match.extension);
        let replacement = format!("@{}", resolve(&name));
        processed.replace_range(code_match.range, &replacement);
    }

    processed
}

#[derive(Debug)]
struct Token {
    range: Range<usize>,
    normalized: String,
}

#[derive(Debug)]
struct CodePhraseMatch {
    range: Range<usize>,
    identifier_words: Vec<String>,
    extension: String,
    last_token_index: usize,
}

fn parse_phrase(tokens: &[Token], gate_index: usize) -> Option<CodePhraseMatch> {
    parse_direct_extension_phrase(tokens, gate_index)
        .or_else(|| parse_spoken_dot_phrase(tokens, gate_index))
}

fn parse_direct_extension_phrase(tokens: &[Token], gate_index: usize) -> Option<CodePhraseMatch> {
    let maximum_last_index =
        (gate_index + MAXIMUM_IDENTIFIER_WORD_COUNT).min(tokens.len().saturating_sub(1));

    for last_index in (gate_index + 1)..=maximum_last_index {
        let Some((identifier, extension_token)) =
            split_identifier_and_extension(&tokens[last_index].normalized)
        else {
            continue;
        };
        let Some(extension) = canonical_extension(extension_token) else {
            continue;
        };

        let mut words: Vec<String> = tokens[(gate_index + 1)..last_index]
            .iter()
            .map(|token| token.normalized.clone())
            .filter(|word| !word.is_empty())
            .collect();

        if words.iter().any(|word| word == "the") {
            continue;
        }

        let Some(mut trailing_words) = identifier_words(identifier) else {
            continue;
        };
        words.append(&mut trailing_words);

        if words.is_empty() || words.len() > MAXIMUM_IDENTIFIER_WORD_COUNT {
            continue;
        }

        return Some(CodePhraseMatch {
            range: tokens[gate_index + 1].range.start..tokens[last_index].range.end,
            identifier_words: words,
            extension: extension.to_owned(),
            last_token_index: last_index,
        });
    }

    None
}

fn parse_spoken_dot_phrase(tokens: &[Token], gate_index: usize) -> Option<CodePhraseMatch> {
    if tokens.len() < gate_index + 4 {
        return None;
    }

    let maximum_dot_index = (gate_index + MAXIMUM_IDENTIFIER_WORD_COUNT + 1).min(tokens.len() - 2);
    let dot_index =
        ((gate_index + 1)..=maximum_dot_index).find(|index| tokens[*index].normalized == "dot")?;
    let identifier_tokens = &tokens[(gate_index + 1)..dot_index];

    if identifier_tokens.is_empty() || identifier_tokens.len() > MAXIMUM_IDENTIFIER_WORD_COUNT {
        return None;
    }

    let words: Vec<String> = identifier_tokens
        .iter()
        .map(|token| token.normalized.clone())
        .filter(|word| !word.is_empty())
        .collect();

    if words.is_empty() || words.iter().any(|word| word == "the") {
        return None;
    }

    let extension_index = dot_index + 1;
    let extension = canonical_extension(&tokens[extension_index].normalized)?;

    Some(CodePhraseMatch {
        range: tokens[gate_index + 1].range.start..tokens[extension_index].range.end,
        identifier_words: words,
        extension: extension.to_owned(),
        last_token_index: extension_index,
    })
}

fn split_identifier_and_extension(token: &str) -> Option<(&str, &str)> {
    let mut parts = token.split('.');
    let identifier = parts.next()?;
    let extension = parts.next()?;

    if identifier.is_empty() || extension.is_empty() || parts.next().is_some() {
        return None;
    }

    Some((identifier, extension))
}

fn identifier_words(identifier: &str) -> Option<Vec<String>> {
    if !identifier
        .chars()
        .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
    {
        return None;
    }

    let words = if identifier.contains('_') {
        identifier.split('_').map(str::to_owned).collect()
    } else if identifier.contains('-') {
        identifier.split('-').map(str::to_owned).collect()
    } else {
        split_camel_or_pascal_case(identifier)
    };

    let words: Vec<String> = words.into_iter().filter(|word| !word.is_empty()).collect();
    (!words.is_empty()).then_some(words)
}

fn split_camel_or_pascal_case(identifier: &str) -> Vec<String> {
    let characters: Vec<char> = identifier.chars().collect();
    let Some(first) = characters.first() else {
        return Vec::new();
    };

    let mut words = Vec::new();
    let mut current = first.to_string();

    for index in 1..characters.len() {
        let character = characters[index];
        let previous = characters[index - 1];
        let next = characters.get(index + 1).copied();
        let starts_new_word = (character.is_uppercase() && previous.is_lowercase())
            || (character.is_uppercase()
                && previous.is_uppercase()
                && next.is_some_and(char::is_lowercase));

        if starts_new_word {
            words.push(current);
            current = character.to_string();
        } else {
            current.push(character);
        }
    }

    words.push(current);
    words
}

fn format_identifier(words: &[String], style: IdentifierStyle) -> String {
    let words: Vec<String> = words
        .iter()
        .map(|word| {
            word.chars()
                .filter(|character| character.is_alphanumeric())
                .collect()
        })
        .filter(|word: &String| !word.is_empty())
        .collect();

    match style {
        IdentifierStyle::Pascal => words.iter().map(|word| capitalize(word)).collect(),
        IdentifierStyle::Camel => {
            let Some((head, tail)) = words.split_first() else {
                return String::new();
            };
            head.to_lowercase() + &tail.iter().map(|word| capitalize(word)).collect::<String>()
        }
        IdentifierStyle::Snake => words
            .iter()
            .map(|word| word.to_lowercase())
            .collect::<Vec<_>>()
            .join("_"),
        IdentifierStyle::Kebab => words
            .iter()
            .map(|word| word.to_lowercase())
            .collect::<Vec<_>>()
            .join("-"),
    }
}

fn capitalize(word: &str) -> String {
    let lowercase = word.to_lowercase();
    let mut characters = lowercase.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
        .unwrap_or_default()
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(text, start, index, &mut tokens);
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }

    if let Some(start) = token_start {
        push_token(text, start, text.len(), &mut tokens);
    }

    tokens
}

fn push_token(text: &str, start: usize, end: usize, tokens: &mut Vec<Token>) {
    let raw = &text[start..end];
    let trimmed = raw.trim_matches(is_edge_punctuation);
    if trimmed.is_empty() {
        return;
    }

    let core_start = start
        + raw
            .find(trimmed)
            .expect("trimmed token belongs to raw token");
    let core_end = core_start + trimmed.len();
    tokens.push(Token {
        range: core_start..core_end,
        normalized: trimmed.to_lowercase(),
    });
}

fn is_edge_punctuation(character: char) -> bool {
    character.is_whitespace() || character.is_ascii_punctuation()
}
