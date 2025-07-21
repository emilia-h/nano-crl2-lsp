
pub struct EditorConfig {
    pub editor: Editor,
    pub check_parse_errors_continuously: bool,
    pub check_errors_continuously: bool,
}

#[derive(Eq, Debug, PartialEq)]
pub enum Editor {
    VsCode,
}
