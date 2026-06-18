use super::*;

impl PreprocSourceMap {
    pub fn insert_real_file(&mut self, source: PreprocSourceId, file_id: FileId, text_len: usize) {
        self.entries.insert(source, PreprocSourceMapping::RealFile(file_id));
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, 0);
    }

    pub fn insert_virtual_file(
        &mut self,
        source: PreprocSourceId,
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
    ) {
        self.insert_virtual_file_with_offset(source, file_id, path, origin, text_len, 0);
    }

    pub(in crate::base_db::source_db::preproc) fn insert_virtual_file_with_offset(
        &mut self,
        source: PreprocSourceId,
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
        range_offset: usize,
    ) {
        let mapping = match file_id {
            Some(file_id) => PreprocSourceMapping::VirtualFile { file_id, path, origin },
            None => PreprocSourceMapping::VirtualDisplay { path, origin },
        };
        self.entries.insert(source, mapping);
        self.predefine_sources.remove(&source);
        self.text_lengths.insert(source, text_len);
        self.range_offsets.insert(source, range_offset);
    }

    pub fn insert_unmapped(&mut self, source: PreprocSourceId, reason: SourcePreprocUnavailable) {
        self.entries.insert(source, PreprocSourceMapping::Unmapped(reason));
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

    pub fn get(&self, source: PreprocSourceId) -> Option<&PreprocSourceMapping> {
        self.entries.get(&source)
    }

    pub fn predefine_manifest_source(
        &self,
        source: PreprocSourceId,
    ) -> Option<PreprocManifestSource> {
        self.predefine_sources.get(&source).copied()
    }

    pub fn file_id(&self, source: PreprocSourceId) -> Result<FileId, PreprocSourceMapError> {
        match self.get(source) {
            Some(PreprocSourceMapping::RealFile(file_id)) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { file_id, .. }) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualDisplay { path, origin }) => {
                Err(PreprocSourceMapError::DisplayOnlyVirtualSource {
                    path: path.clone(),
                    origin: origin.clone(),
                })
            }
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                Err(PreprocSourceMapError::UnmappedSource { source, reason: reason.clone() })
            }
            None => Err(PreprocSourceMapError::MissingSource { source }),
        }
    }

    pub fn source_positions_for_file_offset(
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
                    PreprocSourceMapping::VirtualDisplay { .. } => return None,
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

    pub fn map_range(&self, source_range: SourceRange) -> Result<TextRange, PreprocSourceMapError> {
        match self.get(source_range.source) {
            Some(PreprocSourceMapping::RealFile(_))
            | Some(PreprocSourceMapping::VirtualFile { .. })
            | Some(PreprocSourceMapping::VirtualDisplay { .. }) => {}
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                return Err(PreprocSourceMapError::UnmappedSource {
                    source: source_range.source,
                    reason: reason.clone(),
                });
            }
            None => {
                return Err(PreprocSourceMapError::MissingSource { source: source_range.source });
            }
        }

        let range_offset = self.range_offsets.get(&source_range.source).copied().unwrap_or(0);
        let mapped_range = shift_text_range(source_range.range, range_offset).ok_or(
            PreprocSourceMapError::RangeOutOfBounds {
                source: source_range.source,
                range: source_range.range,
                mapped_range: source_range.range,
                text_len: usize::MAX,
            },
        )?;
        let text_len = self
            .text_lengths
            .get(&source_range.source)
            .copied()
            .ok_or(PreprocSourceMapError::MissingSource { source: source_range.source })?;
        if usize::from(mapped_range.end()) <= text_len {
            return Ok(mapped_range);
        }

        Err(PreprocSourceMapError::RangeOutOfBounds {
            source: source_range.source,
            range: source_range.range,
            mapped_range,
            text_len,
        })
    }
}
