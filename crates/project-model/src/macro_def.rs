use rustc_hash::FxHashSet;
use smol_str::SmolStr;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum MacroAtom {
    Flag(SmolStr),
    KeyValue { key: SmolStr, value: SmolStr },
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct MacroDef {
    pub macros: FxHashSet<MacroAtom>,
}

impl MacroDef {
    pub fn to_predefine_strings(&self) -> Vec<String> {
        let mut predefines = self
            .macros
            .iter()
            .map(|macro_atom| match macro_atom {
                MacroAtom::Flag(name) => name.to_string(),
                MacroAtom::KeyValue { key, value } => format!("{key}={value}"),
            })
            .collect::<Vec<_>>();
        predefines.sort();
        predefines
    }
}
