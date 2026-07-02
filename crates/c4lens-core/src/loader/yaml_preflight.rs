use crate::CommandError;

pub(super) fn reject_anchor_and_alias_tokens(contents: &str) -> Result<(), CommandError> {
    let mut block_scalar_parent_indent = None;
    let mut plain_scalar_parent_indent = None;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for (line_index, line) in contents.lines().enumerate() {
        let line_indent = leading_space_count(line);
        if let Some(parent_indent) = block_scalar_parent_indent {
            if line.trim().is_empty() || line_indent > parent_indent {
                continue;
            }
            block_scalar_parent_indent = None;
        }
        if let Some(parent_indent) = plain_scalar_parent_indent {
            if line.trim().is_empty() || line_indent > parent_indent {
                continue;
            }
            plain_scalar_parent_indent = None;
        }

        let mut escaped = false;
        let mut starts_block_scalar = None;
        let chars: Vec<(usize, char)> = line.char_indices().collect();

        for (char_index, (byte_index, character)) in chars.iter().enumerate() {
            if in_double_quote && escaped {
                escaped = false;
                continue;
            }

            match character {
                '\\' if in_double_quote => {
                    escaped = true;
                }
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                }
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                }
                '#' if !in_single_quote
                    && !in_double_quote
                    && is_yaml_comment_start(&chars, char_index) =>
                {
                    break;
                }
                '&' | '*' if !in_single_quote && !in_double_quote => {
                    if is_yaml_anchor_or_alias_token(line, &chars, char_index) {
                        return Err(CommandError::with_details(
                            "parse.unsupported_yaml_feature",
                            "YAML anchors and aliases are unsupported.",
                            serde_json::json!({
                                "line": line_index + 1,
                                "column": byte_index + 1,
                            }),
                        ));
                    }
                }
                '|' | '>'
                    if !in_single_quote
                        && !in_double_quote
                        && is_yaml_block_scalar_token(line, &chars, char_index) =>
                {
                    starts_block_scalar = Some(block_scalar_parent_indent_for_line(
                        line, &chars, char_index,
                    ));
                }
                _ => {}
            }
        }

        if let Some(parent_indent) = starts_block_scalar {
            block_scalar_parent_indent = Some(parent_indent);
            plain_scalar_parent_indent = None;
        } else if !in_single_quote && !in_double_quote {
            plain_scalar_parent_indent = plain_scalar_parent_indent_for_line(line, &chars);
        }
    }

    Ok(())
}

fn leading_space_count(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn block_scalar_parent_indent_for_line(
    line: &str,
    chars: &[(usize, char)],
    marker_index: usize,
) -> usize {
    let line_indent = leading_space_count(line);
    let Some((byte_index, _)) = chars.get(marker_index) else {
        return line_indent;
    };

    let prefix_after_indent = line[line_indent..*byte_index].trim_end();
    let Some(after_dash) = prefix_after_indent.strip_prefix('-') else {
        return line_indent;
    };
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if separation_spaces == 0 {
        return line_indent;
    }

    if after_dash[separation_spaces..].contains(':') {
        line_indent + 1 + separation_spaces
    } else {
        line_indent
    }
}

fn plain_scalar_parent_indent_for_line(line: &str, chars: &[(usize, char)]) -> Option<usize> {
    let line_indent = leading_space_count(line);
    let comment_start = comment_start_byte(line, chars);
    let significant = line[..comment_start].trim_end();
    if significant.trim().is_empty() {
        return None;
    }

    if let Some(colon_byte) = mapping_value_colon(chars, comment_start) {
        let value = line[colon_byte + 1..comment_start].trim_start();
        if is_plain_scalar_value(value) {
            return Some(mapping_parent_indent_for_colon(line, colon_byte));
        }
        return None;
    }

    let after_indent = &significant[line_indent..];
    let after_dash = after_indent.strip_prefix('-')?;
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if separation_spaces == 0 {
        return None;
    }

    is_plain_scalar_value(&after_dash[separation_spaces..]).then_some(line_indent)
}

fn comment_start_byte(line: &str, chars: &[(usize, char)]) -> usize {
    for (char_index, (byte_index, character)) in chars.iter().enumerate() {
        if *character == '#' && is_yaml_comment_start(chars, char_index) {
            return *byte_index;
        }
    }

    line.len()
}

fn mapping_value_colon(chars: &[(usize, char)], comment_start: usize) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for (char_index, (byte_index, character)) in chars.iter().enumerate() {
        if *byte_index >= comment_start {
            break;
        }
        if in_double_quote && escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' if in_double_quote => {
                escaped = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ':' if !in_single_quote && !in_double_quote => {
                let next = chars.get(char_index + 1).map(|(_, character)| *character);
                if next.is_none_or(|character| character.is_whitespace()) {
                    return Some(*byte_index);
                }
            }
            _ => {}
        }
    }

    None
}

fn mapping_parent_indent_for_colon(line: &str, colon_byte: usize) -> usize {
    let line_indent = leading_space_count(line);
    let prefix_after_indent = line[line_indent..colon_byte].trim_end();
    let Some(after_dash) = prefix_after_indent.strip_prefix('-') else {
        return line_indent;
    };
    let separation_spaces = after_dash
        .chars()
        .take_while(|character| *character == ' ')
        .count();

    if separation_spaces > 0 && !after_dash[separation_spaces..].trim().is_empty() {
        line_indent + 1 + separation_spaces
    } else {
        line_indent
    }
}

fn is_plain_scalar_value(value: &str) -> bool {
    let mut value = value.trim_start();
    while value.starts_with('!') {
        let Some(tag_end) = value.find(char::is_whitespace) else {
            return false;
        };
        value = value[tag_end..].trim_start();
    }

    !value.is_empty()
        && !matches!(
            value.chars().next(),
            Some('\'' | '"' | '|' | '>' | '[' | '{' | '&' | '*')
        )
}

fn is_yaml_anchor_or_alias_token(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, marker)) = chars.get(marker_index) else {
        return false;
    };
    if *marker != '&' && *marker != '*' {
        return false;
    }

    let next = line[*byte_index + marker.len_utf8()..].chars().next();
    if !matches!(next, Some(character) if character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return false;
    }

    if let Some((_, previous)) = marker_index
        .checked_sub(1)
        .and_then(|index| chars.get(index))
    {
        if !previous.is_whitespace() && !is_yaml_token_boundary(*previous) {
            return false;
        }
    }

    let previous = chars[..marker_index]
        .iter()
        .rev()
        .find_map(|(_, character)| (!character.is_whitespace()).then_some(*character));

    previous.is_none()
        || matches!(previous, Some(':' | '[' | '{' | ',' | '-' | '?'))
        || previous_token_is_yaml_tag(line, chars, marker_index)
}

fn is_yaml_block_scalar_token(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, marker)) = chars.get(marker_index) else {
        return false;
    };
    if *marker != '|' && *marker != '>' {
        return false;
    }

    if let Some((_, previous)) = marker_index
        .checked_sub(1)
        .and_then(|index| chars.get(index))
    {
        if !previous.is_whitespace() && !is_yaml_token_boundary(*previous) {
            return false;
        }
    }

    let previous = chars[..marker_index]
        .iter()
        .rev()
        .find_map(|(_, character)| (!character.is_whitespace()).then_some(*character));
    if previous.is_some()
        && !matches!(previous, Some(':' | '[' | '{' | ',' | '-' | '?'))
        && !previous_token_is_yaml_tag(line, chars, marker_index)
    {
        return false;
    }

    let mut rest = line[*byte_index + marker.len_utf8()..].chars().peekable();
    if matches!(rest.peek(), Some('+' | '-')) {
        rest.next();
    }
    while matches!(rest.peek(), Some(character) if character.is_ascii_digit()) {
        rest.next();
    }
    let remainder = rest.collect::<String>();
    let remainder = remainder.trim_start();

    remainder.is_empty() || remainder.starts_with('#')
}

fn is_yaml_comment_start(chars: &[(usize, char)], marker_index: usize) -> bool {
    chars[..marker_index]
        .last()
        .is_none_or(|(_, character)| character.is_whitespace())
}

fn is_yaml_token_boundary(character: char) -> bool {
    matches!(character, ':' | '[' | '{' | ',' | '-' | '?')
}

fn previous_token_is_yaml_tag(line: &str, chars: &[(usize, char)], marker_index: usize) -> bool {
    let Some((byte_index, _)) = chars.get(marker_index) else {
        return false;
    };
    let before = line[..*byte_index].trim_end();
    let token_start = before
        .rfind(char::is_whitespace)
        .map(|index| index + 1)
        .unwrap_or(0);

    before[token_start..].starts_with('!')
}

#[cfg(test)]
mod tests {
    use super::reject_anchor_and_alias_tokens;

    #[test]
    fn rejects_anchor_tokens_with_location() {
        let error =
            reject_anchor_and_alias_tokens("name: Example\nsystems: &systems\n").unwrap_err();

        assert_eq!(error.code, "parse.unsupported_yaml_feature");
        assert_eq!(
            error.details,
            Some(serde_json::json!({
                "line": 2,
                "column": 10
            }))
        );
    }

    #[test]
    fn allows_anchor_like_text_inside_quotes_comments_and_block_scalars() {
        reject_anchor_and_alias_tokens(
            r#"
name: "Quoted *not_alias &not_anchor"
# *commented_alias &commented_anchor
description: |
  *not_alias
  &not_anchor
"#,
        )
        .expect("anchor-like text should be allowed");
    }
}
