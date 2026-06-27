use super::*;

impl PreprocSourceMap {
    pub(crate) fn insert_real_file(
        &mut self,
        source: PreprocSourceId,
        file_id: FileId,
        text_len: usize,
    ) {
        self.entries.insert(source, PreprocSourceMapping::RealFile(file_id));
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, 0);
    }

    pub(crate) fn insert_virtual_file(
        &mut self,
        source: PreprocSourceId,
        file_id: FileId,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
    ) {
        self.insert_virtual_file_with_offset(source, file_id, path, origin, text_len, 0);
    }

    pub(in crate::base_db::source_db::preproc) fn insert_virtual_file_with_offset(
        &mut self,
        source: PreprocSourceId,
        file_id: FileId,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
        range_offset: usize,
    ) {
        self.entries.insert(source, PreprocSourceMapping::VirtualFile { file_id, path, origin });
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, range_offset);
    }

    pub(crate) fn insert_unmapped(
        &mut self,
        source: PreprocSourceId,
        reason: SourcePreprocUnavailable,
    ) {
        self.entries.insert(source, PreprocSourceMapping::Unmapped(reason.into()));
        self.predefine_sources.remove(&source);
        self.text_lengths.remove(&source);
        self.range_offsets.remove(&source);
    }

    pub(in crate::base_db::source_db::preproc) fn insert_predefine_manifest_source(
        &mut self,
        source: PreprocSourceId,
        manifest_source: PreprocManifestSource,
    ) {
        self.predefine_sources.insert(source, manifest_source);
    }

    pub(crate) fn get(&self, source: PreprocSourceId) -> Option<&PreprocSourceMapping> {
        self.entries.get(&source)
    }

    pub(crate) fn predefine_manifest_source(
        &self,
        source: PreprocSourceId,
    ) -> Option<PreprocManifestSource> {
        self.predefine_sources.get(&source).copied()
    }

    pub(crate) fn file_id(&self, source: PreprocSourceId) -> Result<FileId, PreprocSourceMapError> {
        self.file_id_for_mapping(source, self.get(source))
    }

    pub(crate) fn source_positions_for_file_offset(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourcePosition> {
        let mut positions = self
            .entries
            .iter()
            .filter_map(|(source, mapping)| {
                let mapped_file_id = match mapping {
                    PreprocSourceMapping::RealFile(mapped_file_id)
                    | PreprocSourceMapping::VirtualFile { file_id: mapped_file_id, .. } => {
                        *mapped_file_id
                    }
                    PreprocSourceMapping::Unmapped(_) => return None,
                };
                if mapped_file_id != file_id {
                    return None;
                }

                let range_offset = self.range_offsets.get(source).copied().unwrap_or(0);
                let source_offset = unshift_text_size(offset, range_offset)?;
                let text_len = self.text_lengths.get(source).copied()?;
                (usize::from(source_offset) <= text_len)
                    .then_some(SourcePosition { source: *source, offset: source_offset })
            })
            .collect::<Vec<_>>();
        positions.sort_by_key(|position| position.source.raw());
        positions
    }

    pub(crate) fn map_range(
        &self,
        source_range: SourceRange,
    ) -> Result<(FileId, TextRange), PreprocSourceMapError> {
        let mapping = self.get(source_range.source);
        let file_id = self.file_id_for_mapping(source_range.source, mapping)?;

        let range_offset = self.range_offsets.get(&source_range.source).copied().unwrap_or(0);
        let mapped_range = shift_text_range(source_range.range, range_offset).ok_or(
            PreprocSourceMapError::RangeOutOfBounds {
                buffer_id: source_range.source.raw(),
                range: source_range.range,
                mapped_range: source_range.range,
                text_len: usize::MAX,
            },
        )?;
        let text_len =
            self.text_lengths.get(&source_range.source).copied().ok_or(
                PreprocSourceMapError::MissingSource { buffer_id: source_range.source.raw() },
            )?;
        if usize::from(mapped_range.end()) <= text_len {
            return Ok((file_id, mapped_range));
        }

        Err(PreprocSourceMapError::RangeOutOfBounds {
            buffer_id: source_range.source.raw(),
            range: source_range.range,
            mapped_range,
            text_len,
        })
    }

    pub(crate) fn map_buffer_range(
        &self,
        range: &SourceBufferRange,
    ) -> Option<Result<(FileId, TextRange), PreprocSourceMapError>> {
        let source_range = source_range_from_buffer_range(range)?;
        Some(self.map_range(source_range))
    }

    fn file_id_for_mapping(
        &self,
        source: PreprocSourceId,
        mapping: Option<&PreprocSourceMapping>,
    ) -> Result<FileId, PreprocSourceMapError> {
        match mapping {
            Some(PreprocSourceMapping::RealFile(file_id)) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { file_id, .. }) => Ok(*file_id),
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                Err(PreprocSourceMapError::UnmappedSource {
                    buffer_id: source.raw(),
                    reason: reason.clone(),
                })
            }
            None => Err(PreprocSourceMapError::MissingSource { buffer_id: source.raw() }),
        }
    }
}

fn source_range_from_buffer_range(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}
