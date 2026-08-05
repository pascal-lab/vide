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

/// Options for collecting parser grammar expectations.
struct ExpectedSyntaxOptions {
    /// Character offset within the parsed source buffer where expectations should be recorded.
    /// Only used when `recordAll` is false.
    std::optional<size_t> cursorOffset;

    /// Record every expectation site with its real source window instead of
    /// gating on a single cursor offset. The authoritative parse uses this so
    /// one parse serves completion requests at any caret position.
    bool recordAll = false;
};

/// A grammar expectation observed by the parser.
struct ExpectedSyntax {
    /// The parser diagnostic category associated with this expectation.
    DiagCode code = DiagCode();

    /// The specific token expected, when the parser was expecting one fixed token.
    TokenKind tokenKind = TokenKind::Unknown;

    /// The start of the source window where the expectation was recorded.
    /// In single-cursor mode this is the requested cursor offset.
    SourceLocation location = SourceLocation::NoLocation;

    /// The end (exclusive) of the source window where the expectation was
    /// recorded. In single-cursor mode this equals the cursor offset.
    size_t end = 0;

    /// Keyword item context associated with the expectation, when applicable.
    std::optional<syntax::SyntaxKeywordContext> keywordContext;
};

} // namespace slang::parsing
