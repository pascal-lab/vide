use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use crate::{
    alloc_with_source_entry,
    expr::ExprId,
    lower::{BodyStore, LoweringCtx, LoweringStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElabSystemTaskKind {
    Fatal,
    Error,
    Warning,
    Info,
    StaticAssert,
}

impl ElabSystemTaskKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "$fatal" => Some(Self::Fatal),
            "$error" => Some(Self::Error),
            "$warning" => Some(Self::Warning),
            "$info" => Some(Self::Info),
            "$static_assert" => Some(Self::StaticAssert),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElabSystemTaskArgument {
    Ordered(ExprId),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElabSystemTask {
    pub kind: ElabSystemTaskKind,
    pub arguments: SmallVec<[ElabSystemTaskArgument; 4]>,
}

pub type ElabSystemTaskId = Idx<ElabSystemTask>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_elab_system_task(
        &mut self,
        declaration: ast::ElabSystemTask<'_>,
    ) -> Option<ElabSystemTaskId> {
        let Some(name) = declaration.name() else {
            self.report_invalid(
                declaration.syntax(),
                "elaboration system task is missing its name",
            );
            return None;
        };
        let task_name = name.value_text().to_string();
        let Some(kind) = ElabSystemTaskKind::from_name(&task_name) else {
            self.report_invalid(
                declaration.syntax(),
                "elaboration system task has an unknown name",
            );
            return None;
        };

        let arguments = match declaration.arguments() {
            Some(argument_list) => {
                let mut arguments = SmallVec::new();
                for argument in argument_list.parameters().children() {
                    match argument {
                        ast::Argument::OrderedArgument(argument) => {
                            arguments.push(ElabSystemTaskArgument::Ordered(
                                self.lower_property_expr(argument.expr()),
                            ));
                        }
                        ast::Argument::EmptyArgument(_) => {
                            arguments.push(ElabSystemTaskArgument::Empty);
                        }
                        ast::Argument::NamedArgument(argument) => {
                            self.report_unsupported(
                                argument.syntax(),
                                "named arguments are not allowed for elaboration system tasks",
                            );
                            return None;
                        }
                    }
                }
                arguments
            }
            None => SmallVec::new(),
        };

        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.elab_system_tasks,
            &mut sources.elab_system_task_srcs,
            ElabSystemTask { kind, arguments },
            source,
        ))
    }
}
