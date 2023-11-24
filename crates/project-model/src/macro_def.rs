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
