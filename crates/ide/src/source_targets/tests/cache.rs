use super::*;

#[test]
fn source_target_request_cache_reuses_origin_lookup_for_repeated_reference_hits() {
    let mut cache = SourceTargetRequestCache::default();
    let mut lookups = 0usize;
    let file_id = FileId::from_raw(0);
    let offset = TextSize::from(12);

    for _ in 0..3 {
        let result = cache.macro_files_at_offset_with(file_id, offset, || {
            lookups += 1;
            Vec::new()
        });
        assert!(result.is_empty());
    }

    assert_eq!(lookups, 1, "repeated text hits at one offset should reuse the request cache");

    let _ = cache.macro_files_at_offset_with(file_id, offset + TextSize::from(1), || {
        lookups += 1;
        Vec::new()
    });
    assert_eq!(lookups, 2, "different offsets should remain distinct cache entries");
}
