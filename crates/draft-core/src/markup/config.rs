/// Dynamic configuration options set by the `\file` macro or by `config.mgon`.
///
/// These options can be changed at any point within a markup file by a macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynConf {
    pub latex_math: bool,  // `latex` todo
    pub code_lang: String, // `code` todo
}

/// Static configuration options set using compiler flags or by `config.mgon`.
///
/// These options cannot be changed from within a markup file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticConf {
    /// If true, does not recognize inline math formatting to make writing finances easier.
    pub finance_mode: bool,//todo

    /// If true, does not perform a first pass to ensure the input is valid UTF-8.
    pub trusted_mode: bool,

    /// If true, recognizes links without having to use link syntax. todo
    pub infer_links: bool,
}
