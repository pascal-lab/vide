# DefId and Name Resolution

Name resolution must distinguish two cases:

- one definition appears at multiple source locations;
- one name matches multiple competing definitions.

The first case is a definition with multiple origins. The second is an ambiguous resolution. They must not share the same representation.

## Core types

### `DefOrigin`

`DefOrigin` identifies one concrete source origin, such as a module, declaration, or port label.

It owns source-level operations:

- locating source ranges;
- finding the containing scope;
- projecting to HIR nodes through methods such as `as_decl()` and `as_module()`.

### `DefId`

`DefId` identifies one logical definition.

Most definitions have one origin. A definition with several source representations has a `primary_origin` and additional origins.

`primary_origin` is the stable representative of the definition. It is not necessarily the target for “go to declaration”; callers should use `declaration_origin()` for that purpose.

### `Resolution<T>`

`Resolution<T>` describes the result of name resolution:

```rust
enum Resolution<T> {
    Unresolved,
    Unique(T),
    Ambiguous(SmallVec<[T; 2]>),
}
```

- `Unresolved`: no definition was found;
- `Unique`: exactly one result was found;
- `Ambiguous`: several competing results were found.

Candidates in `Ambiguous` must be different `DefId` values, not different origins of the same `DefId`.

## Non-ANSI port example

```systemverilog
module m(a);
  output a;
  reg [7:0] a;
endmodule
```

The three occurrences of `a` are source origins of one logical port:

```text
DefId(port a)
├── primary_origin: a in the module header
├── origin: output a
└── origin: reg [7:0] a
```

Resolving `a` therefore produces:

```text
Unique(DefId(port a))
```

Two unrelated declarations with the same name instead produce two definitions:

```text
Ambiguous([DefId(first), DefId(second)])
```

## Usage rules

1. Never use `first()` to silently choose an ambiguous candidate.
2. Merge source locations into origins only when they represent the same logical definition.
3. Select an origin explicitly before accessing a concrete HIR node:

   ```rust
   def.primary_origin(db).as_decl(db)
   ```

4. Features that require one target must call `unique()`:
   - type inference returns unknown for ambiguous input;
   - rename, references, and document highlight stop on ambiguity;
   - go to definition, go to declaration, and hover may show every candidate.

## Invariants

- Every `DefId` has at least one origin.
- Selecting `primary_origin` must not depend on salsa intern IDs or collection ordering.
- `NameScope::lookup()` and path resolution return `Resolution<DefId>`.
- Source-level `as_*` projections belong to `DefOrigin`, not `DefId`.

## Open question

The current intern key includes additional origins. A future revision may intern only the logical identity and compute origins on demand, so adding or removing an auxiliary declaration does not change the definition ID.
