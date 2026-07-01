use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use utils::{line_index::TextRange, paths::AbsPathBuf};
use workspace_model::project::{Predefine, PredefineSource};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum MacroAtom {
    Flag(SmolStr),
    KeyValue { key: SmolStr, value: SmolStr },
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct MacroDef {
    pub macros: FxHashSet<MacroAtom>,
    pub sources: Vec<MacroDefSource>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MacroDefSource {
    pub atom: MacroAtom,
    pub range: TextRange,
}

impl MacroDef {
    pub fn to_predefine_strings(&self) -> Vec<String> {
        self.to_predefines(None).into_iter().map(|predefine| predefine.definition).collect()
    }

    pub fn to_predefines(&self, manifest_path: Option<&AbsPathBuf>) -> Vec<Predefine> {
        let mut predefines = self
            .macros
            .iter()
            .map(|macro_atom| {
                let definition = macro_atom.predefine_string();
                let source = manifest_path.and_then(|path| {
                    let mut matches = self
                        .sources
                        .iter()
                        .filter(|source| source.atom == *macro_atom)
                        .map(|source| source.range);
                    let range = matches.next()?;
                    matches.next().is_none().then(|| PredefineSource { path: path.clone(), range })
                });
                Predefine { definition, source }
            })
            .collect::<Vec<_>>();
        predefines.sort_by(|left, right| left.definition.cmp(&right.definition));
        predefines
    }
}

impl MacroAtom {
    fn predefine_string(&self) -> String {
        match self {
            MacroAtom::Flag(name) => name.to_string(),
            MacroAtom::KeyValue { key, value } => format!("{key}={value}"),
        }
    }
}
