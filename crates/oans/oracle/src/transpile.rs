use std::path::Path;

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{HelperLoaderMode, TransformOptions, Transformer};

pub fn transpile(path: &Path, source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(format!("{}: {}", path.display(), parsed.diagnostics[0]));
    }
    let mut program = parsed.program;
    let semantic = SemanticBuilder::new().with_excess_capacity(2.0).with_enum_eval(true).build(&program);
    if !semantic.diagnostics.is_empty() {
        return Err(format!("{}: {}", path.display(), semantic.diagnostics[0]));
    }
    let scoping = semantic.semantic.into_scoping();
    let mut options = TransformOptions::from_target("es2021").map_err(|e| e.to_string())?;
    options.helper_loader.mode = HelperLoaderMode::External;
    let transformed = Transformer::new(&allocator, path, &options).build_with_scoping(scoping, &mut program);
    if !transformed.diagnostics.is_empty() {
        return Err(format!("{}: {}", path.display(), transformed.diagnostics[0]));
    }
    Ok(Codegen::new().build(&program).code)
}
