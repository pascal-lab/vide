use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Default)]
pub struct Tag {
    base: Option<String>,
    is_final: bool,
    multi_kind: bool,
}

#[derive(Debug, Clone)]
pub enum Ty {
    Token,
    TokenList,
    SyntaxList(String),
    SeparatedList(String),
    NotNull(String),
    SyntaxNode(String),
}

#[derive(Debug, Default)]
pub struct TypeInfo {
    is_final: bool,
    base: String,
    combined: Vec<(Ty, String)>,
    members: Vec<(Ty, String)>,
}

fn reader_from_file(path: &Path) -> BufReader<File> {
    let file = File::open(path).expect("Unable to open syntax.txt");
    BufReader::new(file)
}

fn trim_name(prefix_len: usize, name: &str) -> &str {
    let name = &name[prefix_len..];
    const C: [char; 3] = ['<', '>', '?'];
    name.trim_matches(C.as_ref())
}

pub mod loader {
    use std::{
        collections::{BTreeMap, HashMap},
        fs,
        io::BufRead,
        path::Path,
    };

    use inflector::Inflector;
    use itertools::Itertools;
    use regex::Regex;

    use crate::sourcegen::{Tag, Ty, TypeInfo, reader_from_file, trim_name};

    pub fn load_types() -> (HashMap<String, TypeInfo>, BTreeMap<String, String>) {
        let mut all_types = HashMap::new();
        let mut kind_map = BTreeMap::new();
        all_types.insert("SyntaxNode".to_owned(), TypeInfo::default());

        let path = Path::new("scripts/syntax.txt");
        let mut lines = reader_from_file(path).lines().map_while(Result::ok);

        let is_end_line = |l: &str| l.is_empty() || l == "empty";

        while let Some(start_line) = lines.next() {
            let start_line = start_line.trim();
            if start_line.is_empty() || start_line.starts_with("//") {
                continue;
            }

            const KINDMAP_PREFIX: &str = "kindmap";
            if start_line.starts_with(KINDMAP_PREFIX) {
                let kind_name = trim_name(KINDMAP_PREFIX.len(), start_line);
                lines.by_ref().take_while(|line| !is_end_line(line)).for_each(|line| {
                    line.split_whitespace().for_each(|kind| {
                        kind_map.insert(kind.to_owned(), kind_name.to_owned());
                    })
                });
            } else {
                let mut parts = start_line.split_whitespace();
                let ty = parts.next().unwrap();

                let mut tag = Tag { is_final: true, ..Default::default() };

                for (name, value) in parts.map(|p| p.split('=').collect_tuple().unwrap()) {
                    match name {
                        "base" => tag.base = Some(value.to_owned()),
                        "final" => tag.is_final = value == "true",
                        "multiKind" => tag.multi_kind = value == "true",
                        _ => unreachable!("Unknown tag {name}"),
                    }
                }

                if tag.is_final && !tag.multi_kind {
                    kind_map.insert(ty.to_owned(), ty.to_owned());
                }

                let base_members = tag
                    .base
                    .as_ref()
                    .filter(|base| *base != "SyntaxNode")
                    .map(|base| all_types.get(base).unwrap().members.clone());

                let members = lines
                    .by_ref()
                    .take_while(|l| !is_end_line(l))
                    .map(|l| {
                        let (ty, member) = l.split_whitespace().collect_tuple::<(_, _)>().unwrap();

                        const LIST_PREFIX: &str = "list";
                        const SEPARATED_LIST_PREFIX: &str = "separated_list";

                        let ty_name = |prefix_len| trim_name(prefix_len, ty).to_owned();

                        let ty = match ty {
                            "token" => Ty::Token,
                            "tokenlist" => Ty::TokenList,
                            _ if ty.starts_with(LIST_PREFIX) => {
                                Ty::SyntaxList(ty_name(LIST_PREFIX.len()))
                            }
                            _ if ty.starts_with(SEPARATED_LIST_PREFIX) => {
                                Ty::SeparatedList(ty_name(SEPARATED_LIST_PREFIX.len()))
                            }
                            _ if !ty.ends_with("?") => Ty::NotNull(ty_name(0)),
                            _ => Ty::SyntaxNode(ty_name(0)),
                        };

                        (ty, member.to_owned())
                    })
                    .collect_vec();

                all_types.insert(
                    ty.to_string(),
                    TypeInfo {
                        is_final: tag.is_final,
                        base: tag.base.unwrap_or_else(|| "SyntaxNode".to_owned()),
                        combined: base_members
                            .into_iter()
                            .flatten()
                            .chain(members.iter().cloned())
                            .collect(),
                        members,
                    },
                );
            }
        }

        assert!(lines.next().is_none());

        (all_types, kind_map)
    }

    pub fn load_token_macros() -> std::io::Result<String> {
        let file_path = Path::new("source/parsing/LexerFacts.cpp");
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let mut pairs = Vec::new();
        let mut in_target_function = false;

        let case_re =
            Regex::new(r#"case\s+TokenKind::(\w+):\s+return\s+"((?:\\.|[^"])*)";"#).unwrap();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if !in_target_function {
                if line.contains("LexerFacts::getTokenKindText") {
                    in_target_function = true;
                    while i < lines.len() && !lines[i].contains('{') {
                        i += 1;
                    }
                }
            } else {
                if line.contains("default: return \"\";") {
                    break;
                }

                if let Some(cap) = case_re.captures(line) {
                    let token_name = cap[1].to_string();
                    let token_value = cap[2].to_string();
                    pairs.push((token_value, token_name));
                }
            }
            i += 1;
        }

        let mut macro_str = String::from("#[macro_export]\n");
        macro_str.push_str("macro_rules! Token { \n");
        let special_chars = ['(', ')', '[', ']', '{', '}', '\'', '"', '`', '$'];
        for (value, name) in pairs {
            if value.chars().any(|c| special_chars.contains(&c)) {
                macro_str.push_str(&format!(
                    "    [\"{}\"] => {{ $crate::TokenKind::{} }}; \n",
                    value,
                    name.to_screaming_snake_case()
                ));
            } else if value == "##" {
                macro_str.push_str(&format!(
                    "    [# #] => {{ $crate::TokenKind::{} }}; \n",
                    name.to_screaming_snake_case()
                ));
            } else {
                macro_str.push_str(&format!(
                    "    [{}] => {{ $crate::TokenKind::{} }}; \n",
                    value,
                    name.to_screaming_snake_case()
                ));
            }
        }

        macro_str.push('}');

        Ok(macro_str)
    }
}

pub mod generator {
    use std::{
        collections::{BTreeMap, HashMap, HashSet},
        env,
        ffi::OsString,
        fs,
        io::{self, BufRead, Write},
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };

    use inflector::Inflector;
    use itertools::Itertools;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, format_ident, quote};

    use super::{Ty, TypeInfo, reader_from_file};

    // From https://docs.rs/bindgen/0.51.1/src/bindgen/lib.rs.html#1945
    fn rustfmt_path() -> io::Result<OsString> {
        if let Ok(rustfmt) = env::var("RUSTFMT") {
            return Ok(OsString::from(rustfmt));
        }

        which::which("rustfmt")
            .map_or_else(|e| Err(io::Error::other(format!("{}", e))), |p| Ok(p.into_os_string()))
    }

    fn mkdir_and_write(file: &Path, contents: String) -> Result<(), std::io::Error> {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }

        let rustfmt = rustfmt_path().expect("Failed to find rustfmt");
        let mut cmd = Command::new(&*rustfmt);
        cmd.arg("--config").arg("use_small_heuristics=Max");

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let mut child = cmd.spawn().unwrap();
        let mut child_stdin = child.stdin.take().unwrap();
        let mut child_stdout = child.stdout.take().unwrap();

        let stdin_handle = std::thread::spawn(move || {
            let _ = child_stdin.write_all(contents.as_bytes());
            contents
        });

        let mut output = vec![];
        io::copy(&mut child_stdout, &mut output).expect("Failed to read rustfmt output");

        let status = child.wait().unwrap();
        let contents =
            stdin_handle.join().expect("The thread writing to rustfmt's stdin doesn't do any");

        match (String::from_utf8(output), status.code()) {
            (Ok(output), Some(0)) => fs::write(file, output),
            (Ok(_), Some(2)) => {
                panic!("Rustfmt parsing errors");
            }
            (Ok(_), Some(3)) => {
                panic!("Rustfmt could not format some lines");
            }
            (Ok(_), _) => {
                panic!("Internal rustfmt errors");
            }
            (Err(_), _) => fs::write(file, contents),
        }
    }

    fn out_dir(file: &str) -> PathBuf {
        let out_dir = env::var_os("OUT_DIR").unwrap();
        Path::new(&out_dir).join(file)
    }

    pub fn generate_syntax_kind(kind_map: &BTreeMap<String, String>) {
        fn generate_kinds<T: TryFrom<usize> + ToTokens>(
            iter: impl IntoIterator<Item = String>,
        ) -> (Vec<TokenStream>, Vec<TokenStream>) {
            iter.into_iter()
                .enumerate()
                .map(|(id, kind)| {
                    let kind_name = format_ident!("{}", kind.to_screaming_snake_case());
                    let id = T::try_from(id).ok().unwrap();
                    let def = quote! {
                        pub const #kind_name: Self = Self(#id);
                    };
                    let debug_match_arm = quote! {
                        Self::#kind_name => #kind,
                    };
                    (def, debug_match_arm)
                })
                .unzip()
        }

        let syntax_kind_names = {
            let kinds = kind_map.keys().map(|kind| kind.to_string());
            let list_kinds = ["Unknown", "SyntaxList", "TokenList", "SeparatedList"]
                .into_iter()
                .map(String::from);
            list_kinds.chain(kinds).collect::<Vec<_>>()
        };

        let syntax_kind_all = syntax_kind_names.iter().map(|kind| {
            let kind_name = format_ident!("{}", kind.to_screaming_snake_case());
            quote! {
                Self::#kind_name
            }
        });

        let (syntax_kinds, syntax_debug_arms) = {
            let (syntax_kinds, syntax_debug_arms) =
                generate_kinds::<u16>(syntax_kind_names.iter().cloned());
            (syntax_kinds.into_iter(), syntax_debug_arms.into_iter())
        };

        let (token_kinds, token_debug_arms) = {
            let token_path = Path::new("scripts/tokenkinds.txt");
            let lines = reader_from_file(token_path).lines().map_while(Result::ok);
            let (token_kinds, token_debug_arms) = generate_kinds::<u16>(lines);
            (token_kinds.into_iter(), token_debug_arms.into_iter())
        };

        let (trivia_kinds, trivia_debug_arms) = {
            let trivia_path = Path::new("scripts/triviakinds.txt");
            let lines = reader_from_file(trivia_path).lines().map_while(Result::ok);
            let (trivia_kinds, trivia_debug_arms) = generate_kinds::<u8>(lines);
            (trivia_kinds.into_iter(), trivia_debug_arms.into_iter())
        };

        let res = quote! {
            use std::fmt;

            #[derive(Clone, Copy, PartialEq, Eq, Hash)]
            pub struct SyntaxKind(u16);
            impl SyntaxKind {
                #(#syntax_kinds)*

                pub const ALL: &'static [Self] = &[#(#syntax_kind_all),*];

                pub fn is_list(&self) -> bool {
                    *self == Self::SYNTAX_LIST || *self == Self::TOKEN_LIST || *self == Self::SEPARATED_LIST
                }

                pub fn from_id(id: u16) -> Self {
                    Self(id)
                }
            }

            impl fmt::Debug for SyntaxKind {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let name = match *self {
                        #(#syntax_debug_arms)*
                        _ => unreachable!(),
                    };
                    f.write_str(name)
                }
            }

            #[derive(Clone, Copy, PartialEq, Eq, Hash)]
            pub struct TokenKind(u16);
            impl TokenKind {
                #(#token_kinds)*

                pub fn from_id(id: u16) -> Self {
                    Self(id)
                }
            }

            impl fmt::Debug for TokenKind {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let name = match *self {
                        #(#token_debug_arms)*
                        _ => unreachable!(),
                    };
                    f.write_str(name)
                }
            }

            #[derive(Clone, Copy, PartialEq, Eq, Hash)]
            pub struct TriviaKind(u8);
            impl TriviaKind {
                #(#trivia_kinds)*

                pub fn from_id(id: u8) -> Self {
                    Self(id)
                }
            }

            impl fmt::Debug for TriviaKind {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let name = match *self {
                        #(#trivia_debug_arms)*
                        _ => unreachable!(),
                    };
                    f.write_str(name)
                }
            }
        }
        .to_string();

        let path = out_dir("syntax_kind.rs");
        mkdir_and_write(path.as_path(), res).expect("Failed to write syntax_kind.rs");
    }

    fn reverse_map(
        all_types: &HashMap<String, TypeInfo>,
        kind_map: &BTreeMap<String, String>,
    ) -> (HashMap<String, HashSet<String>>, HashMap<String, HashSet<String>>) {
        let mut reversed_map = HashMap::new();
        let mut base_map = HashMap::new();

        for (k, v) in kind_map {
            reversed_map.entry(v.to_owned()).or_insert_with(HashSet::new).insert(k.to_owned());
            base_map.entry(v.to_owned()).or_insert_with(HashSet::new).insert(k.to_owned());
        }

        for (mut k, mut v) in all_types.iter().filter(|(_, v)| v.is_final) {
            while v.base != "SyntaxNode" {
                let kinds = reversed_map.get(k).unwrap().clone();
                reversed_map.entry(v.base.clone()).or_insert_with(HashSet::new).extend(kinds);
                base_map.entry(v.base.clone()).or_insert_with(HashSet::new).insert(k.clone());
                k = &v.base;
                v = all_types.get(k).unwrap();
            }
        }

        (reversed_map, base_map)
    }

    fn escape_kw(name: String) -> String {
        let reserved = ["type", "use", "let"];
        if reserved.contains(&name.as_str()) { format!("{}_", name) } else { name }
    }

    fn generate_members(ty_info: &TypeInfo) -> impl Iterator<Item = TokenStream> + '_ {
        ty_info.combined.iter().enumerate().map(|(idx, (member_ty, member_name))| {
            let member_name = format_ident!("{}", escape_kw(member_name.to_snake_case()));
            let transform_ty = |mut ty: &str| {
                if ty == "SyntaxNode" {
                    ty = "HybridNode";
                }
                format_ident!("{}", ty)
            };

            match member_ty {
                Ty::Token => {
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> Option<SyntaxToken<'a>> {
                            self.syntax().child_token(#idx)
                        }
                    }
                }
                Ty::TokenList => {
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> TokenList<'a> {
                            self.syntax().child_node(#idx).and_then(TokenList::cast).unwrap()
                        }
                    }
                }
                Ty::SyntaxList(member_ty) => {
                    let member_ty = transform_ty(member_ty);
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> SyntaxList<'a, #member_ty<'a>> {
                            self.syntax().child_node(#idx).and_then(SyntaxList::cast).unwrap()
                        }
                    }
                }
                Ty::SeparatedList(member_ty) => {
                    let member_ty = transform_ty(member_ty);
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> SeparatedList<'a, #member_ty<'a>> {
                            self.syntax().child_node(#idx).and_then(SeparatedList::cast).unwrap()
                        }
                    }
                }
                Ty::NotNull(member_ty) => {
                    let member_ty = transform_ty(member_ty);
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> #member_ty<'a> {
                            self.syntax().child_node(#idx).and_then(#member_ty::cast).unwrap()
                        }
                    }
                }
                Ty::SyntaxNode(member_ty) => {
                    let member_ty = transform_ty(member_ty);
                    quote! {
                        #[inline]
                        pub fn #member_name(&self) -> Option<#member_ty<'a>> {
                            self.syntax().child_node(#idx).and_then(#member_ty::cast)
                        }
                    }
                }
            }
        })
    }

    pub fn generate_ast_file(
        all_types: &HashMap<String, TypeInfo>,
        kind_map: &BTreeMap<String, String>,
    ) {
        let (reverse_kind_map, base_map) = reverse_map(all_types, kind_map);
        let asts = all_types
            .iter()
            .filter(|(ty_name, _)| ty_name.as_str() != "SyntaxNode")
            .map(|(ty_name, ty_info)| {
                let members = generate_members(ty_info);

                let ty = format_ident!("{ty_name}");
                let kinds = reverse_kind_map.get(ty_name).unwrap();
                let bases = base_map.get(ty_name).unwrap();
                if kinds.len() == 1 {
                    let kind_name =
                        format_ident!("{}", kinds.iter().next().unwrap().to_screaming_snake_case());
                    quote! {
                        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
                        pub struct #ty<'a> {
                            syntax: SyntaxNode<'a>,
                        }

                        impl<'a> #ty<'a> {
                            #(#members)*
                        }

                        impl<'a> AstNode<'a> for #ty<'a> {
                            #[inline]
                            fn syntax(&self) -> SyntaxNode<'a> {
                                self.syntax
                            }

                            #[inline]
                            fn can_cast(kind: SyntaxKind) -> bool {
                                kind == SyntaxKind::#kind_name
                            }

                            #[inline]
                            fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
                                Self::can_cast(syntax.kind()).then_some(Self { syntax })
                            }
                        }
                    }
                } else {
                    let mut processed_bases = HashSet::new();
                    let (variants, syntax, as_fns): (Vec<_>, Vec<_>, Vec<_>) = kinds
                        .iter()
                        .filter_map(|kind_name| {
                            let variant_name = if let Some(base) = bases
                                .iter()
                                .find(|&base| reverse_kind_map.get(base).is_some_and(|children| children.contains(kind_name))) {
                                    if all_types.get(kind_name).is_none() && kind_map.get(kind_name).unwrap() == ty_name {
                                        kind_name
                                    } else if processed_bases.insert(base) {
                                        base
                                    } else {
                                        return None;
                                    }
                                } else {
                                    kind_name
                                };
                            let kind = format_ident!("{variant_name}");
                            let as_fn = format_ident!("as_{}", variant_name.to_snake_case());

                            if all_types.get(variant_name).is_some() && ty_name != variant_name {
                                Some((
                                    quote! { #kind(#kind<'a>) },
                                    quote! { Self::#kind(node) => node.syntax() },
                                    quote! {
                                        #[inline]
                                        pub fn #as_fn(self) -> Option<#kind<'a>> {
                                            match self {
                                                Self::#kind(node) => Some(node),
                                                _ => None,
                                            }
                                        }
                                    },
                                ))
                            } else {
                                Some((
                                    quote! { #kind(SyntaxNode<'a>) },
                                    quote! { Self::#kind(node) => *node },
                                    quote! {
                                        #[inline]
                                        pub fn #as_fn(self) -> Option<SyntaxNode<'a>> {
                                            match self {
                                                Self::#kind(node) => Some(node),
                                                _ => None,
                                            }
                                        }
                                    },
                                ))
                            }
                        }).multiunzip();

                    let casts = kinds.iter().map(|kind_name| {
                        let kind_id = format_ident!("{}", kind_name.to_screaming_snake_case());
                        let variant_name = if let Some(base) = bases
                            .iter()
                            .find(|&base| reverse_kind_map.get(base).is_some_and(|children| children.contains(kind_name))) {
                                if all_types.get(kind_name).is_none() && kind_map.get(kind_name).unwrap() == ty_name {
                                    kind_name
                                } else {
                                    base
                                }
                        } else {
                            kind_name
                        };
                        let variant = format_ident!("{}", variant_name);

                        if all_types.get(variant_name).is_some() && ty_name != variant_name {
                            quote! { SyntaxKind::#kind_id => Some(Self::#variant(#variant::cast(syntax).unwrap())) }
                        } else {
                            quote! { SyntaxKind::#kind_id => Some(Self::#variant(syntax)) }
                        }
                    });

                    let (variants, syntax) = (variants.into_iter(), syntax.into_iter());

                    let kinds = kinds.iter().map(|kind| {
                        let kind_name = format_ident!("{}", kind.to_screaming_snake_case());
                        quote! { kind == SyntaxKind::#kind_name }
                    });

                    quote! {
                        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
                        pub enum #ty<'a> {
                            #(#variants),*
                        }

                        impl<'a> #ty<'a> {
                            #(#members)*
                            #(#as_fns)*
                        }

                        impl<'a> AstNode<'a> for #ty<'a> {
                            #[inline]
                            fn syntax(&self) -> SyntaxNode<'a> {
                                match self {
                                    #(#syntax),*
                                }
                            }

                            #[inline]
                            fn can_cast(kind: SyntaxKind) -> bool {
                                #(#kinds)||*
                            }

                            #[inline]
                            fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
                                match syntax.kind() {
                                    #(#casts,)*
                                    _ => None,
                                }
                            }
                        }
                    }
                }
            });

        let res = quote! { #(#asts)* };
        let path = out_dir("ast.rs");
        mkdir_and_write(path.as_path(), res.to_string()).expect("Failed to write ast.rs");
    }

    pub(crate) fn generate_token_macro(res: String) {
        let path = out_dir("token.rs");
        mkdir_and_write(path.as_path(), res.to_string()).expect("Failed to write token.rs");
    }
}
