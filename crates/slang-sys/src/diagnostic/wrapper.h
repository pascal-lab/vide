#pragma once

#include "../wrapper.h"
#include "syntax/wrapper.h"

namespace slang_sys::diagnostic {
    struct RawSyntaxDiagnostic;

    rust::Vec<RawSyntaxDiagnostic> diagnostics_to_rust(
        const ::slang::Diagnostics& diagnostics,
        const ::slang::SourceManager& source_manager,
        rust::Vec<rust::String> warning_options
    );
} // namespace slang_sys::diagnostic

namespace slang_sys::diagnostic::tree {

    rust::Vec<RawSyntaxDiagnostic> syntax_tree_diagnostics(
        const syntax::SyntaxTree &tree,
        rust::Vec<rust::String> warning_options
    );

} // namespace slang_sys::diagnostic::tree
