use super::tokens::highlight;

#[test]
fn highlighting_preserves_script_bytes_and_html_as_text() {
    let source = "#!/bin/sh\n# <script>alert('x')</script>\nexport LABEL=\"résumé 🐈 & <img>\"\nprintf '%s\\n' \"$LABEL\"\n";
    let tokens = highlight(source);
    assert_eq!(
        tokens.iter().map(|token| token.text).collect::<String>(),
        source
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.text == "export" && token.class == "shell-keyword")
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.text.contains("<script>") && token.class == "shell-comment")
    );
}

#[test]
fn multiline_and_unfinished_quotes_remain_single_strings() {
    for source in [
        "'first\nsecond'",
        "\"first\\\"\nsecond\"",
        "'unfinished 🐈",
        "\"unfinished\\",
    ] {
        let tokens = highlight(source);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].class, "shell-string");
        assert_eq!(tokens[0].text, source);
    }
}

#[test]
fn heredocs_keep_body_comments_and_quotes_literal_until_delimiter() {
    let source =
        "cat <<'EOF'\n# literal \"unfinished\n<file>$VALUE</file>\nEOF\nif true; then exit; fi\n";
    let tokens = highlight(source);
    assert_eq!(
        tokens.iter().map(|token| token.text).collect::<String>(),
        source
    );
    assert!(tokens.iter().any(|token| token.class == "shell-string"
        && token.text == "# literal \"unfinished\n<file>$VALUE</file>\nEOF\n"));
    assert!(
        tokens
            .iter()
            .any(|token| token.class == "shell-keyword" && token.text == "if")
    );
}

#[test]
fn queued_and_tab_indented_heredocs_stop_at_matching_delimiters() {
    let source = "cat <<A <<-\"B\"\nfirst\nA\n\tsecond\n\tB\nexit\n";
    let tokens = highlight(source);
    assert_eq!(
        tokens.iter().map(|token| token.text).collect::<String>(),
        source
    );
    for body in ["first\nA\n", "\tsecond\n\tB\n"] {
        assert!(
            tokens
                .iter()
                .any(|token| token.class == "shell-string" && token.text == body)
        );
    }
    assert!(
        tokens
            .iter()
            .any(|token| token.text == "exit" && token.class == "shell-keyword")
    );
}

#[test]
fn escaped_operators_and_here_strings_do_not_start_heredocs() {
    let source = "echo \\#literal \\🐈\ncat <<<word\nexit\n";
    let tokens = highlight(source);
    assert_eq!(
        tokens.iter().map(|token| token.text).collect::<String>(),
        source
    );
    assert!(
        tokens
            .iter()
            .all(|token| token.class != "shell-comment" && token.class != "shell-string")
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.class == "shell-keyword" && token.text == "exit")
    );
}

#[test]
fn large_or_token_dense_scripts_fall_back_to_plain_text() {
    for source in ["x".repeat(128 * 1024 + 1), ";".repeat(8193)] {
        let tokens = highlight(&source);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].class, "");
        assert_eq!(tokens[0].text, source);
    }
}
