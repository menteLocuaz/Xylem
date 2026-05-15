use serde::{Serialize, Deserialize};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryType {
    Highlights,
    Locals,
    Folds,
    Injections,
    TextObjects,
    Indents,
    Conceal,
}

impl QueryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryType::Highlights => "highlights",
            QueryType::Locals => "locals",
            QueryType::Folds => "folds",
            QueryType::Injections => "injections",
            QueryType::TextObjects => "textobjects",
            QueryType::Indents => "indents",
            QueryType::Conceal => "conceal",
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HighlightKind {
    Function,
    Variable,
    Number,
    String,
    Comment,
    Keyword,
    Normal,
}

impl HighlightKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            HighlightKind::Function => "Function",
            HighlightKind::Variable => "Variable",
            HighlightKind::Number => "Number",
            HighlightKind::String => "String",
            HighlightKind::Comment => "Comment",
            HighlightKind::Keyword => "Keyword",
            HighlightKind::Normal => "Normal",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "function" | "method" | "function.builtin" | "function.call" | "constructor" => HighlightKind::Function,
            "variable" | "variable.parameter" | "variable.builtin" | "variable.other" | "property" | "field" => HighlightKind::Variable,
            "number" | "float" | "integer" => HighlightKind::Number,
            "string" | "string.regex" | "string.escape" | "string.special" => HighlightKind::String,
            "comment" | "comment.line" | "comment.block" | "comment.documentation" => HighlightKind::Comment,
            "keyword" | "keyword.control" | "keyword.operator" | "keyword.function" | "keyword.return" | "conditional" | "repeat" | "debug" | "exception" | "include" | "storageclass" | "structure" | "type.qualifier" => HighlightKind::Keyword,
            _ => HighlightKind::Normal,
        }
    }
}

impl std::fmt::Display for HighlightKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct QueryKey {
    pub lang: String,
    pub query_type: QueryType,
}
