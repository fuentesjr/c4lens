#[derive(Clone, Debug, Default)]
pub(super) struct ParsedArtifacts {
    pub(super) symbols: Vec<ParsedSymbol>,
    pub(super) imports: Vec<ParsedImport>,
}

#[derive(Clone, Debug)]
pub(super) struct ParsedSymbol {
    pub(super) kind: &'static str,
    pub(super) name: String,
    pub(super) qualified_name: Option<String>,
    pub(super) start_line: i32,
    pub(super) start_column: i32,
    pub(super) end_line: i32,
    pub(super) end_column: i32,
}

#[derive(Clone, Debug)]
pub(super) struct ParsedImport {
    pub(super) target_module: String,
    pub(super) target_path: Option<String>,
    pub(super) kind: &'static str,
}
