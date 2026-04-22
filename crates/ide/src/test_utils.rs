pub(crate) fn normalize_fixture_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
