use crate::state::visible_name;

#[test]
fn filenames_keep_spaces_and_unicode_but_show_control_and_direction_characters() {
    assert_eq!(
        visible_name("review résumé 日本語.md"),
        "review résumé 日本語.md"
    );
    assert_eq!(
        visible_name("line\nnext\t\u{1b}[31m"),
        "line\\nnext\\t\\u{1b}[31m"
    );
    assert_eq!(visible_name("safe\u{202e}gpj.exe"), "safe\\u{202e}gpj.exe");
}
