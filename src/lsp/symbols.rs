//! Language-agnostic symbol types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A symbol found in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name (e.g. "MyStruct", "my_function")
    pub name: String,
    /// Fully qualified name path (e.g. "MyModule/MyStruct/my_method")
    pub name_path: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    /// 0-indexed start line
    pub start_line: u32,
    /// 0-indexed end line
    pub end_line: u32,
    /// 0-indexed start column
    pub start_col: u32,
    /// Children symbols (e.g. methods of a class)
    pub children: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Unknown,
}

impl From<lsp_types::SymbolKind> for SymbolKind {
    fn from(k: lsp_types::SymbolKind) -> Self {
        use lsp_types::SymbolKind as L;
        match k {
            L::FILE => Self::File,
            L::MODULE => Self::Module,
            L::NAMESPACE => Self::Namespace,
            L::PACKAGE => Self::Package,
            L::CLASS => Self::Class,
            L::METHOD => Self::Method,
            L::PROPERTY => Self::Property,
            L::FIELD => Self::Field,
            L::CONSTRUCTOR => Self::Constructor,
            L::ENUM => Self::Enum,
            L::INTERFACE => Self::Interface,
            L::FUNCTION => Self::Function,
            L::VARIABLE => Self::Variable,
            L::CONSTANT => Self::Constant,
            L::STRING => Self::String,
            L::NUMBER => Self::Number,
            L::BOOLEAN => Self::Boolean,
            L::ARRAY => Self::Array,
            L::OBJECT => Self::Object,
            L::KEY => Self::Key,
            L::NULL => Self::Null,
            L::ENUM_MEMBER => Self::EnumMember,
            L::STRUCT => Self::Struct,
            L::EVENT => Self::Event,
            L::OPERATOR => Self::Operator,
            L::TYPE_PARAMETER => Self::TypeParameter,
            _ => Self::Unknown,
        }
    }
}
