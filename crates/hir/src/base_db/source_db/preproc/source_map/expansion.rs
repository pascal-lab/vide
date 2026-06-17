use super::*;

impl PreprocSourceMap {
    pub fn insert_expansion_display_only(
        &mut self,
        expansion: SourceMacroExpansionId,
        path: VfsPath,
        display_text: String,
        emitted_range: SourceEmittedTokenRange,
        display_token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    ) {
        self.expansion_entries.insert(
            expansion,
            PreprocExpansionMapping {
                origin: PreprocVirtualOrigin::Expansion { expansion },
                emitted_range,
                display: PreprocExpansionDisplay {
                    path: path.clone(),
                    text: display_text,
                    token_ranges: display_token_ranges,
                },
                source_buffer: PreprocExpansionSourceBuffer::DisplayOnly { path },
            },
        );
    }

    pub fn expansion(&self, expansion: SourceMacroExpansionId) -> Option<&PreprocExpansionMapping> {
        self.expansion_entries.get(&expansion)
    }

    pub fn expansion_display_source(
        &self,
        expansion: SourceMacroExpansionId,
    ) -> Result<PreprocSourceMapping, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        Ok(PreprocSourceMapping::VirtualDisplay {
            path: entry.display.path.clone(),
            origin: entry.origin.clone(),
        })
    }

    pub fn expansion_source_buffer(
        &self,
        expansion: SourceMacroExpansionId,
    ) -> Result<PreprocSourceMapping, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        Ok(match &entry.source_buffer {
            PreprocExpansionSourceBuffer::ParseStable { file_id, path, .. } => {
                PreprocSourceMapping::VirtualFile {
                    file_id: *file_id,
                    path: path.clone(),
                    origin: entry.origin.clone(),
                }
            }
            PreprocExpansionSourceBuffer::DisplayOnly { path } => {
                PreprocSourceMapping::VirtualDisplay {
                    path: path.clone(),
                    origin: entry.origin.clone(),
                }
            }
        })
    }

    pub fn emitted_display_range(
        &self,
        expansion: SourceMacroExpansionId,
        emitted_range: SourceEmittedTokenRange,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        emitted_range_from_token_ranges(&entry.display.token_ranges, emitted_range)
            .ok_or(PreprocSourceMapError::MissingEmittedTokenRange { range: emitted_range })
    }

    pub fn emitted_source_buffer_range(
        &self,
        expansion: SourceMacroExpansionId,
        emitted_range: SourceEmittedTokenRange,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        let PreprocExpansionSourceBuffer::ParseStable { token_ranges, .. } = &entry.source_buffer
        else {
            return Err(display_only_expansion_source_buffer_error(entry));
        };
        emitted_range_from_token_ranges(token_ranges, emitted_range)
            .ok_or(PreprocSourceMapError::MissingEmittedTokenRange { range: emitted_range })
    }

    pub fn emitted_token_display_range(
        &self,
        expansion: SourceMacroExpansionId,
        token: SourceEmittedTokenId,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        entry
            .display
            .token_ranges
            .get(&token)
            .copied()
            .ok_or(PreprocSourceMapError::MissingEmittedToken { token })
    }

    pub fn emitted_token_source_buffer_range(
        &self,
        expansion: SourceMacroExpansionId,
        token: SourceEmittedTokenId,
    ) -> Result<TextRange, PreprocSourceMapError> {
        let entry = self
            .expansion(expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        let PreprocExpansionSourceBuffer::ParseStable { token_ranges, .. } = &entry.source_buffer
        else {
            return Err(display_only_expansion_source_buffer_error(entry));
        };
        token_ranges
            .get(&token)
            .copied()
            .ok_or(PreprocSourceMapError::MissingEmittedToken { token })
    }

    pub fn insert_expansion_parse_stable_source_buffer(
        &mut self,
        expansion: SourceMacroExpansionId,
        file_id: FileId,
        path: VfsPath,
        text: String,
        token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    ) -> Result<(), PreprocSourceMapError> {
        let entry = self
            .expansion_entries
            .get_mut(&expansion)
            .ok_or(PreprocSourceMapError::MissingExpansionVirtualFile { expansion })?;
        entry.source_buffer =
            PreprocExpansionSourceBuffer::ParseStable { file_id, path, text, token_ranges };
        Ok(())
    }

    pub fn expansion_display_text(&self, expansion: SourceMacroExpansionId) -> Option<&str> {
        self.expansion(expansion).map(|entry| entry.display.text.as_str())
    }
}
