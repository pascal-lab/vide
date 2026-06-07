//------------------------------------------------------------------------------
// PreprocessorTrace.h
// Shared preprocessor trace facts
//
// SPDX-FileCopyrightText: Michael Popoloski
// SPDX-License-Identifier: MIT
//------------------------------------------------------------------------------
#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <vector>

#include "slang/parsing/Token.h"

namespace slang {

struct SourceBuffer;

namespace syntax {
class SyntaxNode;
struct DefineDirectiveSyntax;
struct MacroActualArgumentListSyntax;
} // namespace syntax

namespace parsing {

/// A macro usage observed by the preprocessor while expanding source tokens.
struct MacroUsageTraceRecord {
    Token directive;
    syntax::MacroActualArgumentListSyntax* actualArgs = nullptr;
    SourceRange range;
    uint32_t callId = 0;
    uint32_t definitionId = 0;
    uint32_t expansionId = 0;
    uint32_t parentExpansionId = 0;
};

struct PreprocessorTraceDirectiveEvent {
    const syntax::SyntaxNode* syntax = nullptr;
    uint32_t macroDefinitionId = 0;
};

struct PreprocessorTraceEvent {
    enum class Kind : uint8_t { Directive, MacroUsage };

    uint32_t eventId = 0;
    Kind kind = Kind::Directive;
    PreprocessorTraceDirectiveEvent directive;
    MacroUsageTraceRecord macroUsage;
};

struct PreprocessorTraceSnapshot {
    std::optional<uint32_t> rootBufferId;
    std::vector<PreprocessorTraceEvent> events;
    std::vector<Token> emittedTokens;
};

class PreprocessorTraceRecorder {
public:
    void setRootBuffer(SourceBuffer buffer);

    void recordDirective(const syntax::SyntaxNode& syntax, uint32_t macroDefinitionId = 0);
    void recordEmittedToken(Token token);
    void flushMacroUsageRecords(std::span<const MacroUsageTraceRecord> records);

    PreprocessorTraceSnapshot snapshot() const { return snapshot_; }

private:
    PreprocessorTraceEvent& pushEvent(PreprocessorTraceEvent::Kind kind);

    PreprocessorTraceSnapshot snapshot_;
    size_t flushedMacroUsageRecordCount_ = 0;
};

} // namespace parsing
} // namespace slang
