//------------------------------------------------------------------------------
//! @file ExpectedSyntax.h
//! @brief Parser expected syntax metadata for editor completion
//
// SPDX-FileCopyrightText: Michael Popoloski
// SPDX-License-Identifier: MIT
//------------------------------------------------------------------------------
#pragma once

#include <cstddef>
#include <optional>

#include "slang/diagnostics/Diagnostics.h"
#include "slang/parsing/Token.h"
#include "slang/syntax/SyntaxFacts.h"
#include "slang/text/SourceLocation.h"

namespace slang::parsing {

/// Options for collecting parser grammar expectations at a source offset.
struct ExpectedSyntaxOptions {
    /// Character offset within the parsed source buffer where expectations should be recorded.
    std::optional<size_t> cursorOffset;
};

/// A grammar expectation observed by the parser at the requested source offset.
struct ExpectedSyntax {
    /// The parser diagnostic category associated with this expectation.
    DiagCode code = DiagCode();

    /// The specific token expected, when the parser was expecting one fixed token.
    TokenKind tokenKind = TokenKind::Unknown;

    /// The source location of the requested offset.
    SourceLocation location = SourceLocation::NoLocation;

    /// Keyword item context associated with the expectation, when applicable.
    std::optional<syntax::SyntaxKeywordContext> keywordContext;
};

} // namespace slang::parsing
