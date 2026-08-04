use super::*;

impl PreprocSourceMap {
    pub fn insert_real_file(&mut self, source: PreprocSourceId, file_id: FileId, text_len: usize) {
        self.entries.insert(
            source,
            SourceEntry {
                mapping: PreprocSourceMapping::RealFile(file_id),
                text_len,
                range_offset: 0,
                manifest_source: None,
            },
        );
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

    pub(in crate::source_db) fn insert_virtual_file_with_offset(
        &mut self,
        source: PreprocSourceId,
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
        text_len: usize,
        range_offset: usize,
    ) {
        self.entries.insert(
            source,
            SourceEntry {
                mapping: PreprocSourceMapping::VirtualFile { file_id, path, origin },
                text_len,
                range_offset,
                manifest_source: None,
            },
        );
    }

    pub fn insert_unmapped(&mut self, source: PreprocSourceId, reason: SourcePreprocUnavailable) {
        self.entries.insert(
            source,
            SourceEntry {
                mapping: PreprocSourceMapping::Unmapped(reason),
                text_len: 0,
                range_offset: 0,
                manifest_source: None,
            },
        );
    }

    pub(in crate::source_db) fn insert_predefine_manifest_source(
        &mut self,
        source: PreprocSourceId,
        manifest_source: PreprocManifestSource,
    ) {
        if let Some(entry) = self.entries.get_mut(&source) {
            entry.manifest_source = Some(manifest_source);
        }
    }

    pub fn get(&self, source: PreprocSourceId) -> Option<&PreprocSourceMapping> {
        self.entries.get(&source).map(|entry| &entry.mapping)
    }

    pub fn predefine_manifest_source(
        &self,
        source: PreprocSourceId,
    ) -> Option<PreprocManifestSource> {
        self.entries.get(&source).and_then(|entry| entry.manifest_source)
    }

    pub fn file_id(&self, source: PreprocSourceId) -> Result<FileId, SourcePreprocQueryError> {
        match self.get(source) {
            Some(PreprocSourceMapping::RealFile(file_id)) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { file_id: Some(file_id), .. }) => Ok(*file_id),
            Some(PreprocSourceMapping::VirtualFile { path, origin, .. }) => {
                Err(SourcePreprocQueryError::DisplayOnlyVirtualSource {
                    path: path.clone(),
                    origin: origin.clone(),
                })
            }
            Some(PreprocSourceMapping::Unmapped(reason)) => {
                Err(SourcePreprocQueryError::SourceUnavailable(reason.clone()))
            }
            None => Err(SourcePreprocQueryError::MissingSource { source }),
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
            .filter_map(|(source, entry)| {
                let mapped_file_id = match &entry.mapping {
                    PreprocSourceMapping::RealFile(mapped_file_id)
                    | PreprocSourceMapping::VirtualFile { file_id: Some(mapped_file_id), .. } => {
                        *mapped_file_id
                    }
                    PreprocSourceMapping::VirtualFile { file_id: None, .. }
                    | PreprocSourceMapping::Unmapped(_) => return None,
                };
                if mapped_file_id != file_id {
                    return None;
                }

                let source_offset = unshift_text_size(offset, entry.range_offset)?;
                (usize::from(source_offset) <= entry.text_len)
                    .then_some(SourcePosition { source: *source, offset: source_offset })
            })
            .collect::<Vec<_>>();
        positions.sort_by_key(|position| position.source.raw());
        positions
    }

    pub fn map_range(
        &self,
        source_range: SourceRange,
    ) -> Result<TextRange, SourcePreprocQueryError> {
        let Some(entry) = self.entries.get(&source_range.source) else {
            return Err(SourcePreprocQueryError::MissingSource { source: source_range.source });
        };
        if let PreprocSourceMapping::Unmapped(reason) = &entry.mapping {
            return Err(SourcePreprocQueryError::SourceUnavailable(reason.clone()));
        }

        let mapped_range = shift_text_range(source_range.range, entry.range_offset).ok_or(
            SourcePreprocQueryError::RangeOutOfBounds {
                source: source_range.source,
                range: source_range.range,
                mapped_range: source_range.range,
                text_len: usize::MAX,
            },
        )?;
        if usize::from(mapped_range.end()) <= entry.text_len {
            return Ok(mapped_range);
        }

        Err(SourcePreprocQueryError::RangeOutOfBounds {
            source: source_range.source,
            range: source_range.range,
            mapped_range,
            text_len: entry.text_len,
        })
    }
}
