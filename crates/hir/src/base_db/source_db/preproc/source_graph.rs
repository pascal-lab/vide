use preproc::source::{PreprocSourceId, SourceIncludeStatus};
use rustc_hash::FxHashMap;
use source_model::{
    IncludeDirectiveId, SourceContext, SourceContextId, SourceDomain, SourceDomainId, SourceGraph,
    SourceGraphBuilder, SourceUnavailable, VirtualOrigin,
};
use vfs::FileId;

use super::{
    MappedSourcePreprocModel, PreprocSourceMapping, PreprocVirtualOrigin, SourcePreprocUnavailable,
};
use crate::base_db::project::CompilationProfileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGraphPreprocModel {
    pub graph: SourceGraph,
    pub root_context: SourceContextId,
    pub source_domains: FxHashMap<PreprocSourceId, SourceDomainId>,
}

pub(super) fn source_graph_preproc_model_from_mapped(
    mapped: &MappedSourcePreprocModel,
    root_file: FileId,
    profile_id: Option<CompilationProfileId>,
) -> SourceGraphPreprocModel {
    let mut builder = SourceGraphBuilder::new();
    let root_context = builder.add_context(SourceContext::CompilationRoot {
        profile_id: profile_id.map(|id| id.0),
        root_file,
    });

    let mut source_domains = FxHashMap::default();
    for (source, mapping) in mapped.source_map.source_mappings() {
        let domain = builder.intern_domain(source_domain_from_preproc_mapping(mapping));
        source_domains.insert(source, domain);
    }

    for include in mapped.model.include_graph().directives() {
        if let SourceIncludeStatus::Resolved { source } = include.status {
            builder.add_context(SourceContext::IncludeContext {
                parent: root_context,
                include_directive: IncludeDirectiveId::new(include.id.raw() as u32),
                included_file: mapped.source_map.file_id(source).unwrap_or(root_file),
            });
        }
    }

    SourceGraphPreprocModel { graph: builder.build(), root_context, source_domains }
}

fn source_domain_from_preproc_mapping(mapping: &PreprocSourceMapping) -> SourceDomain {
    match mapping {
        PreprocSourceMapping::RealFile(file_id) => SourceDomain::RealFile { file_id: *file_id },
        PreprocSourceMapping::VirtualFile { file_id, path, origin } => SourceDomain::VirtualFile {
            file_id: *file_id,
            path: path.clone(),
            origin: virtual_origin_from_preproc(origin),
        },
        PreprocSourceMapping::VirtualDisplay { path, origin } => SourceDomain::VirtualDisplay {
            path: path.clone(),
            origin: virtual_origin_from_preproc(origin),
        },
        PreprocSourceMapping::Unmapped(reason) => {
            SourceDomain::Unmapped { reason: source_unavailable_from_preproc(reason) }
        }
    }
}

fn virtual_origin_from_preproc(origin: &PreprocVirtualOrigin) -> VirtualOrigin {
    match origin {
        PreprocVirtualOrigin::Predefines { profile } => {
            VirtualOrigin::Predefines { profile: profile.map(|id| id.0) }
        }
        PreprocVirtualOrigin::Builtin { name } => VirtualOrigin::Builtin { name: name.clone() },
        PreprocVirtualOrigin::ExternalIncludeBuffer { source } => {
            VirtualOrigin::ExternalIncludeBuffer { source: source.raw() }
        }
        PreprocVirtualOrigin::Expansion { expansion } => VirtualOrigin::Expansion {
            expansion: source_model::MacroExpansionId::new(expansion.raw() as u32),
        },
        PreprocVirtualOrigin::Speculative { universe } => {
            VirtualOrigin::Speculative { universe: universe.0 }
        }
    }
}

fn source_unavailable_from_preproc(reason: &SourcePreprocUnavailable) -> SourceUnavailable {
    match reason {
        SourcePreprocUnavailable::MacroCallAuthorityUnavailable => {
            SourceUnavailable::MacroCallAuthorityUnavailable
        }
        SourcePreprocUnavailable::ExpansionAuthorityUnavailable => {
            SourceUnavailable::ExpansionAuthorityUnavailable
        }
        SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable => {
            SourceUnavailable::TokenProvenanceAuthorityUnavailable
        }
        SourcePreprocUnavailable::DetachedSource { source }
        | SourcePreprocUnavailable::MissingPredefineSourceText { source }
        | SourcePreprocUnavailable::UnverifiedPredefineSource { source } => {
            SourceUnavailable::UnmappedSource { source: source.raw() }
        }
        _ => SourceUnavailable::Unsupported,
    }
}
