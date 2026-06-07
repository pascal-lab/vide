//------------------------------------------------------------------------------
// PreprocessorTrace.cpp
// Shared preprocessor trace facts
//
// SPDX-FileCopyrightText: Michael Popoloski
// SPDX-License-Identifier: MIT
//------------------------------------------------------------------------------
#include "slang/parsing/PreprocessorTrace.h"

#include "slang/text/SourceManager.h"

namespace slang::parsing {

void PreprocessorTraceRecorder::setRootBuffer(SourceBuffer buffer) {
    if (buffer)
        snapshot_.rootBufferId = buffer.id.getId();
}

void PreprocessorTraceRecorder::recordDirective(const syntax::SyntaxNode& syntax,
                                                uint32_t macroDefinitionId, bool isPredefine) {
    auto& event = pushEvent(PreprocessorTraceEvent::Kind::Directive);
    event.directive.syntax = &syntax;
    event.directive.macroDefinitionId = macroDefinitionId;
    event.directive.isPredefine = isPredefine;
}

void PreprocessorTraceRecorder::recordEmittedToken(Token token) {
    snapshot_.emittedTokens.push_back(token);
}

void PreprocessorTraceRecorder::flushMacroUsageRecords(
    std::span<const MacroUsageTraceRecord> records) {
    for (; flushedMacroUsageRecordCount_ < records.size(); flushedMacroUsageRecordCount_++) {
        auto& event = pushEvent(PreprocessorTraceEvent::Kind::MacroUsage);
        event.macroUsage = records[flushedMacroUsageRecordCount_];
    }
}

PreprocessorTraceEvent& PreprocessorTraceRecorder::pushEvent(PreprocessorTraceEvent::Kind kind) {
    auto& event = snapshot_.events.emplace_back();
    event.eventId = uint32_t(snapshot_.events.size() - 1);
    event.kind = kind;
    return event;
}

} // namespace slang::parsing
