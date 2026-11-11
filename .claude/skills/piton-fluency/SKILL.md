---
name: piton-fluency
description: "The Piton fluency reference: the Piton language, project configuration, plugins, Belay, packages, tests and the piton CLI. Use when you read or change .pi files or piton.config.pi, or when you run piton commands"
---

# Piton Fluency

This document is a reference for Piton and its family: the language, project
configuration, plugins, the bundled Belay, test and migrate plugins,
packages and tools. Read it before you read or change a Piton specbase. The
GenerateFluencyPrompt skill makes it from `spec/`. Section 19 gives the
differences between the compiler and the spec.

## 1. What Piton is

Piton is a declarative language for agent guidance. It is a "semi-natural
programming language". Indentation gives its structure. It describes skills,
agents, commands, scoped instructions and reference documents. A value can be
a paragraph of English. A value can also be a typed structure with
inheritance and composition.

The reason for Piton (`spec/scope/founding-thesis/`):

- Each language is an abstraction of the layer below it. Agentic development
  is the next layer, and the agent is the compiler.
- Humans design systems that agree with themselves. Agents write the code.
- A prompt in one session leaves no record that you can trace.
- Pure prose (AGENTS.md, CLAUDE.md) decays. It is not clear, it repeats, and
  you cannot use it again.
- Piton adds imports, inheritance, types and references to prose. You write a
  pattern one time and use it many times.
- The author compares Piton to CAM above G-code. CNC replaced the hand of the
  machinist, not the skill. The problem below is repetition.
- The goal: an agent that reads the spec becomes a compiler for Piton and
  Belay. This repository is that test. `crates/` and `plugins/` are the
  compiler.

Core facts:

- **Piton has no runtime.** It compiles to data (JSON, YAML, Markdown) through
  **renderers**. Plugins with a build, such as Belay, write more output.
- Piton has no functions and no loops. The ternary is the only conditional.
  All values are immutable.
- **Belay is a plugin.** The compiler holds nothing that is special to Belay.
  Its constructs and adapters are Piton files. Its build is TypeScript.
- Plugin code (section 11) runs only for a plugin command, for build plans
  (`piton build`, `piton check`), for reach hooks (`piton reach`), for
  inspect hooks before a command, and when the editor saves a file. It never
  runs for `piton compile` or while you type.
- **The `.pi` source is the truth.** Generated Markdown (`.claude/...`,
  `CLAUDE.md`, `AGENTS.md`, build output) comes from it. Edit the `.pi`
  source and build again. Do not edit the output.
- A **specbase** is a project. It has `piton.config.pi`, a source root
  (usually `spec/`), an entry file, plugins and packages.
- A clean compile shows that guidance has a correct form. It does not show
  that an agent will do the correct thing. `piton test` (section 14) lets an
  agent judge plain English statements.

## 2. Lexical rules

- Source files end in `.pi`. A directory with an `index.pi` is a module.
- **Indentation is structure.** Use tabs or spaces, the same in all of one
  file. The canonical form is 4 spaces. `piton format` writes it, and you
  cannot change this.
- **Comments** start with `//` on their own line. Piton has no block comments.
- After code on a line, `//` is text. `url: https://x.com // note` is one
  string.
- The formatter adds a space after `//` and changes nothing more in a comment.
- To write a line that starts with `//` as text, escape it: `\ // \`.
- **Reserved words:** `anchor abstract export from import use as extends pass
  this self super true false null any simple complex string number boolean
  list dictionary`.
- A reserved word cannot be a keyword (`reserved-keyword`). It can be a key:
  `null: text` and `use: text` are correct.
- Keywords are lowercase and can use kebab-case. You define a keyword with
  `as` (8.4).
- An anchor must have a body. Write `pass` for an anchor with no properties.
  `anchor X:` alone gives `empty-anchor`.

## 3. Declarations and values

Top-level names are variables in the scope of their file. `export` lets other
files import them.

```piton
myVariable: 42
export pi: 3.14
export anchor MyAnchor:
    description: This is my anchor
```

- `key: value` must have a space after the colon.
- The value can be on the same line or indented below. The indentation is
  syntax. It is not part of the string.
- **Each property must have a value.** An empty value gives `empty-value`.
  Write `null` for "no value".
- Only an abstract anchor can declare a constraint without a value
  (`title:: string`). In other places this gives `missing-value`.

### 3.1 Types

Piton has loose types and optional type constraints.

| Type | Notes |
| --- | --- |
| `string` | Unicode. You never put quotes around it. |
| `number` | One 64-bit float type. |
| `boolean` | `true`, `false`. |
| `null` | The only "no value". `{null == null}` is `true`. |
| `list` | Ordered. Items can have different types if no constraint stops it. |
| `dictionary` | String keys. Values can have different types if no constraint stops it. |
| anchor | The only type that users define. |
| reference | The result of `@{}` (6.4). |

Simple types are string, number, boolean and null. Complex types are list,
dictionary and anchor.

Piton infers the type of a value without a constraint from its form:

| Value | Type |
| --- | --- |
| `true` | boolean |
| `true story` | string |
| `null` | null |
| `42`, `-42` | number |
| `42 things` | string |
| `A + B` | string (no braces) |
| `This costs $5 + tax` | string |
| `"false"` | string |
| `Total: {a + b}` | string |
| `{A + B}` | the type that the operands give |
| `${...}` | string |

Each built-in type is a `type` anchor (`spec/scope/language/types/lib/Type.pi`).
It has `description`, `what-is-a-type`, `supportedOperators` and
`unsupportedOperators`. An operator that a type does not list is an error.
Piton applies this check after coercion and inference.

| Type | Operators |
| --- | --- |
| number | `+ - * / %`, `== != < <= > >=` |
| string | `+`, `++`, `== != < <= > >=` |
| boolean | `&& \|\| !`, `==`, `!=` |
| null | `==`, `!=` |
| list | `+`, `++`, `==`, `!=` |
| dictionary | `.`, `+`, `++`, `==`, `!=` |
| anchor | `.`, `==`, `!=` |
| reference | `==`, `!=` |

### 3.2 Numbers

- Correct forms: `123`, `3.14`, `0.14`, `1_200_000.00`, `-5`.
- A decimal must have its leading `0`. The compiler reads `.5` as a string.
- `_` separates digits.
- Piton has no exponent notation. The compiler reads `1e3` as a string and
  gives no error.
- `-5` is a number. `- 5` is a list item.
- Minus is never a prefix operator. `{-price}` is an error. Write
  `{0 - price}`.
- Output uses the shortest form: `1_200_000.00` → `1200000`, `0.50` → `0.5`.

### 3.3 Strings

- Strings have no quotes. **Quotes are usual characters**:
  `greeting: "Hello"` keeps the quotes.
- Only in an expression do you put quotes around string operands
  (`{"Hello" + name}`). There, the quotes are not part of the value.
- In a block of many lines, Piton joins the lines with a space.
- A blank line (or more than one) starts a new paragraph. The spec shows two
  paragraphs with a blank line between them. The compiler writes one `\n`
  (section 19).
- Markdown in a string is plain text. Piton never resolves or checks
  `[x](./x.md)` or `![x](./x.png)`. A Markdown link can never be an error.
- Only `@{}` makes links that Piton checks.

### 3.4 Escapes

- Put special text between backslashes, with a space on each side:
  `\ {1 + 2} \` → `{1 + 2}`, `\ : \` → `:`. Piton removes the spaces.
- This is the only escape. A single backslash (`\:`) is literal text.
- To escape backslashes, use more backslashes. The closing wrapper must be
  the same: `\\ \ {x} \ \\` → `\ {x} \`.
- A line that holds only backslashes opens a verbatim block. The same count
  of backslashes closes it. The spec uses `\\\`.
- A verbatim block keeps its line breaks and indentation as written.

Code fences protect nothing. Expressions, comments, list items and keys in a
fence are parsed. Put an escape block in the fence, as all spec examples do.
Tables also need an escape block, or Piton joins their rows.

````piton
description:
    ```piton
    \\\
    anchor Example:
        value: {1 + 2}
    \\\
    ```
````

An escape changes the parse. It does not change the inferred type.
`\ false \` is still a boolean. Write `${false}` for the string.

**Prose problems.** These change the structure of a block and give no error:

- A line that starts with `- ` is a list item. A line that starts with `+ ` or
  `++ ` is a composition line (5). A `:: string` property then fails. Escape
  the mark: `\ - \ item`.
- A line with the shape `word: text` is a key. Examples are `Inputs: none` and
  a wrapped line that starts with `type:`. The block becomes an implicit list.
  Escape the colon or wrap the line at a different word.
- Backticks do not protect text. `` `{x}` `` is an expression.
- An expression in text that gives a list, dictionary or anchor makes an
  implicit list (6.1).

### 3.5 Lists

```piton
a:
    - One
    - Two
b: [This, is, a, list]
nested:
    - Level 1
        - Level 2
```

`nested` is `[Level 1, [Level 2]]`.

- **Lists have no index access.** `myList[0]` does not exist. You merge and
  inherit lists. You do not calculate with them.
- A list item is never a key. `- Settings:` is the string `"Settings:"`.
- Text indented below an item becomes the *next* item. `- url` with `tag: v1`
  below it gives `["url", {"tag": "v1"}]`. You cannot address that
  dictionary.

### 3.6 Dictionaries

- Indentation makes dictionaries. `{a.b.c}` reads nested keys.
- Keys hold only Unicode letters, Unicode numbers, `_` and `-`. Keys cannot
  hold spaces or dots.
- `123`, `foo-bar`, `false` and `null` are correct keys. A key is always a
  string.
- A hyphen is part of the name: `{config.foo-bar}`. Subtraction must have
  spaces: `{a - b}`.
- If the block of a key holds only a list, that list is the value.
- If other content is above the list, the value is an implicit list. The
  list is one item of it.

### 3.7 Implicit lists

A block with prose, lists and keys together compiles to a list in source
order:

```piton
combined:
    This is a string
    - A
    - B
    nested:
        deep: text
```

The result is `["This is a string", ["A", "B"], {"nested": {"deep": "text"}}]`.
You can address keys that the block declares directly
(`{combined.nested.deep}`). You cannot address the prose or the list items.

## 4. Variables and constraints

- **In an expression, a bare name is a file-level variable or an import.**
  This is also true in an anchor. Use `this.x` or `self.x` for a property of
  the anchor.
- The constraint syntax is `name:: type: value`. A space after `::` and after
  `:` is mandatory (`constraint-spacing`).
- The spec documents `:` (assignment) and `::` (type annotation) as
  declaration operators (`spec/scope/language/operators/declaration/`). They
  are not expression operators.

```piton
count:: number: 42
tags:: string[]: [foo, bar]
config:: dictionary:
    a:: number: 1
x:: simple: 1
y:: complex: [1]
z:: any: 1
```

| Constraint | Accepts |
| --- | --- |
| `simple`, `complex`, `any` | The groups in 3.1. `any` accepts all values. |
| `T[]` | A list of `T`. `T` can be an anchor (`Foo[]`). |
| `Foo` (an anchor) | `Foo` or a copy of `Foo` (`f:: Foo: {Foo}`). |
| `extends Foo`, `extends Foo[]` | Any anchor with `Foo` in its chain (8.3). |

- The spec uses `extends` constraints on exported lists, for example
  `export ARITHMETIC_OPERATORS:: extends ArithmeticOperator[]: ...`.
- To name an abstract anchor as a type, import it:
  `from ./Foo import Foo`. `use` alone is not sufficient.
- Coercion rules apply only to constraints. Operators such as `+` have their
  own rules (6.3).

**More than one constraint.** Piton tries the types from left to right. The
first type that can hold the value wins.

- `a:: number:: string: 42` → `42`.
- `b:: string:: number: 42` → `"42"`.
- A literal that becomes a string keeps its text: `string: 1.0` → `"1.0"`.
- Piton calculates an expression first: `string: {1.0}` → `"1"`.
- **Piton never coerces booleans or null.** `flag:: string: false` gives
  `type-mismatch`. Write `${false}`.
- Piton never coerces collections or anchors.
- If no type matches, the result is `type-mismatch`.

**Required and optional properties** (in abstract anchors):

```piton
abstract anchor Card:
    title:: string
    subtitle:: string:: null
    footer:: string:: null: null
```

- A constraint with no value is required. If an implementer does not write
  it, the result is `unimplemented-property`.
- A required property stays required when null is permitted. You must write
  `subtitle`.
- A default value makes a property optional (`footer`). The LSP does not
  report optional properties that are not there.
- **A child cannot declare an inherited constraint again**
  (`redeclared-constraint`). It can only give a value.

## 5. Composition lines

Piton builds a block from top to bottom. **A line that starts with `+` or `++`
combines all of the block above it with the value of the line.** The line uses
that operator (6.3).

```piton
anchor Base:
    items:
        - A
        - B
anchor Child extends Base:
    items:
        + {super.items}
        - C
```

`Child.items` is `[A, B, C]`. With `- {super.items}`, it is `[[A, B], C]`.

| Operand | `+` | `++` |
| --- | --- | --- |
| Lists | Merge. Remove duplicates and keep the last copy. | Keep all items. |
| Dictionaries | Shallow merge. | Deep merge. |
| Strings | Join with nothing. | Join with `\n`. |

- `Child text.` and then `++ {super.d}` gives `"Child text.\nBase text."`.
- A block of only `+ {x}` is equal to `{x}`.
- To put inherited prose before new text with a space, use interpolation:
  `${super.description} and more`.

## 6. Expressions

**Piton calculates nothing outside braces.** `a + b` is text. `{a + b}` is a
calculation.

- In braces, bare words are symbols. Put quotes around strings, also in list
  literals (`{["A"] + ["B"]}`). An unknown name gives `unresolved-symbol`.
- Put the full expression in the braces. `{this.flag} && false` is text with
  one expression in it.
- An expression that is not correct is an error. The compiler also reports
  empty braces (`{}`, `${}`). Escape braces that are literal text.
- Forward references resolve.
- A loop of values that cannot resolve (`A: {B}`, `B: {A}`) gives
  `cyclic-value`.
- Piton copies collections by value.
- **Piton copies anchors, but a copy keeps its identity.** If `x: {Foo}`, then
  `{x == Foo}` is true, `${x}` is `"Foo"`, and `x` passes a `Foo` constraint.
- Write `@{Foo}` for a link. `{Foo}` gives a copy.

### 6.1 Forms and text

Each form does one job. It does the same job in all renderers.

| Form | Result |
| --- | --- |
| `{e}` | The intrinsic value. The type stays the same. An anchor serializes as its content. |
| `${e}` | A string. The output format escapes it. |
| `#{e}` | A number. Piton changes it to text only if the output format needs text. |
| `@{e}` | A reference. It renders as a link or path, never as content. |

`@{X}` points to a document. `{X}` puts a copy in place. `${X}` gives its name.

A value that is exactly one expression keeps its type. With other text around
the expression:

- `${x}` makes the full value a string.
- `{x}` with a simple value goes into the text: `Total: {a + b}` →
  `"Total: 3"`.
- `{x}` with a list, dictionary or anchor **makes the value an implicit
  list**: `See {Foo} here` → `["See", <Foo copy>, "here"]`. In Markdown, the
  content of `Foo` appears in place below its own headings.
- `@{x}` puts in a link or a path.

### 6.2 Conversions

`${}`:

- A string stays the same. Numbers and booleans become text. Null becomes
  `null`.
- An anchor becomes its source name: `${Child}` → `"Child"`.
- `${self}` becomes the name of the current most-derived anchor.
- Piton never changes the name to title case.
- A named list or dictionary becomes its name (`Anchor.prop`, or the variable
  name).
- An unnamed collection (`${[1, 2]}`) gives `no-string-form`.
- **`${}` never puts the contents in place.** Piton never parses the result as
  source again.

Example. `export anchor Metadata:` has `name: Piton` and
`version:: string: 1.0`.

| Source | Result |
| --- | --- |
| `Info ${Metadata}` | `"Info Metadata"` |
| `Info {Metadata}` | `["Info", {"name": "Piton", "version": "1.0"}]` |
| `Info @{Metadata}` | `"Info ./Metadata.json:Metadata"` |
| `Info @{Metadata}` (Markdown) | `[Metadata](./Metadata.md#metadata)` |

`#{}`:

- A number stays the same.
- A string changes to a number only if the **full** string is a Piton number
  literal. `#{"42"}` → `42`. Piton never calculates the string.
- Booleans, null, collections and anchors give `not-a-number`.
- The value stays a number in structured output.

### 6.3 Operators

| Group | Operators | Rules |
| --- | --- | --- |
| Access | `.` | Dictionaries and anchors. |
| Arithmetic | `+ - * / %` | Numbers only. Division or modulo by zero is an error. `%` keeps the sign of the left side: `{-7 % 3}` → `-1`. |
| Comparison | `== != < <= > >=` | `==` and `!=` work on all types. Order works on two numbers or on two strings (by code point). A mix is an error. |
| Logical | `&& \|\| !` | Booleans only. **No truthiness** (`invalid-operand`). **No short circuit**: Piton calculates both sides. `{true \|\| 5}` and `{false && 1 / 0 == 1}` are errors. A short circuit can only hide errors. |
| Conditional | `c ? a : b` | The condition must be a boolean (`invalid-condition`). Piton calculates only the selected branch. Chains are permitted on both sides. |
| Merge | `+`, `++` | See below. |

`+` selects its operation from the operand types:

| Operands | Result |
| --- | --- |
| number + number | Addition. |
| string and a simple value | Concatenation. The `${}` rules make the text: `{2 + "Hello"}` → `"2Hello"`. |
| string and a list, dictionary or anchor (either side) | An implicit list. Piton never makes a string: `{"Tags: " + tags}` → `["Tags: ", [...]]`. |
| list + list | Merge. Remove duplicates, keep the last copy: `{["A","B","C","D"] + ["A","B","C"]}` → `[D, A, B, C]`. |
| dictionary + dictionary | Shallow merge. The right side wins. |
| other | Error. `{1 + true}`, `{null + 1}` and `{1 ++ 2}` give `invalid-operand`. |

`++`:

- Lists: concatenation with duplicates (`[A, B, C, D, A, B, C]`).
- Dictionaries: deep merge. Piton merges nested dictionaries. In other cases
  the right side wins.
- Strings: join with `\n`.
- `+` is like a set. `++` is like a list.

Equality:

- Piton compares collections by structure.
- Two anchors are equal only if they are the same anchor.
- Two references are equal if they point to the same thing.

Precedence, from high to low: `.` › `!` › `* / %` › `+ - ++` ›
`< <= > >=` › `== !=` › `&&` › `||` › `? :`.
All operators group from the left. The ternary groups from the right.
Parentheses change the order. Examples: `{1 + 2 == 3 && !false}`,
`{a ? b : c ? d : e}`.

### 6.4 References `@{}`

- A reference must name an anchor or **a property of an anchor**
  (`@{Button}`, `@{Button.color}`). Other targets give `invalid-reference`.
- You cannot refer to a top-level list or dictionary. Put it in an anchor.
- The target, or the anchor that owns the property, goes into the dependency
  graph. The build writes it.
- Piton resolves a reference for each output target. If a target cannot show
  it, the result is an error.
- A reference never becomes a copy or a name.
- You can compare references with `==` and `!=`.
- A renderer or a plugin can check references.

| Renderer | `Button` | `Button.color` |
| --- | --- | --- |
| JSON | `"./Button.json:Button"` | `"./Button.json:Button.color"` |
| YAML | `./Button.yaml:Button` | `./Button.yaml:Button.color` |
| Markdown | `[Button](./Button.md#button)` | `[Button.color](./Button.md#color)` |

- A Markdown link is relative to the file that holds the reference.
- It links to the specific anchor when many anchors share one output file.
- **Markdown links are lazy.** Piton does not put the content in place. An
  agent opens a link only when it needs it.

## 7. Modules

```piton
from ./FirstFile import pi, MyAnchor
from ./FirstFile import pi SliceOf, MyAnchor Alias
from /subdir/file import AnAnchor
from .. import Something
from my-package import MyAnchor
from @piton/belay import ClaudeCodeAdapter
from ./file import
    First,
    Second
```

- A path is relative to its file.
- A path that starts with `/` is relative to the config `root`. This works
  only in a project with a `piton.config.pi`.
- `.pi` is optional. The formatter removes it.
- A directory, `.` or `..` means the `index.pi` of that directory.
- You can import only exports. Other names give `unresolved-export`. A module
  that is not there gives `unresolved-module`.
- An alias has the form `Name Alias`. Piton has no `as` in imports. Only the
  alias is in scope.

Exports:

- `export Name` exports again a name that the file declares or imports.
- In an index, the short forms are `from ./X export X`, `from ./X export *`
  and `from ./X export A AliasA, B`.
- **An import into an index does not export the name again.**
- **A module exports each name one time** (`duplicate-export`). This applies
  to a name written out and to a name from `export *`, if the two names mean
  different things.
- The same anchor through two routes is not an error.
- The error names the two sources. Fix it with an alias or an explicit list.
  The LSP shows the same error.
- The compiler keeps one export. The order is: the exported declaration, then
  named exports in order, then `*` exports in order.
- If nothing else reaches the dropped item, `piton reach` reports it as
  unreachable.

Circular imports:

- **Circular imports are permitted.**
- Only values that cannot resolve (`A: {B}`, `B: {A}`) and an anchor that
  extends itself are errors.
- Two anchors that describe each other with `${}` are correct.

`use`:

- **`use` brings only user-defined keywords into scope.** Examples:
  `use ./Keywords`, `use .`, `use my-package`, `use my-package/X`,
  `use @piton/belay`.
- `import` never brings keywords. A keyword without its `use` gives
  `unknown-keyword`.
- To declare with a keyword and also name its abstract as a type, write both
  `use` and `import`.

Bundled packages: `@piton/config`, `@piton/packaging`, `@piton/plugin`, and
one package for each bundled plugin with Piton files: `@piton/belay` and
`@piton/test`. The migrate plugin has no Piton files. Each project has all
of them without configuration.

The formatter sorts and breaks import and export lines:

- In each group of import lines, `use` lines come first, then `from` lines.
- It sorts lines by path and sorts the names.
- A blank line separates groups.
- It breaks a line across lines if it has more than 2 items or more than 80
  columns.

## 8. Anchors

An anchor is a named declaration. It has a part in inheritance, types,
exports and composition. It is like a class that you never make an instance
of. It compiles to an object of its resolved properties. Its keys follow the
dictionary rules.

### 8.1 Inheritance

- `anchor C extends A, B:` copies the properties of each base, then the
  properties of the child.
- **The base at the right wins. The child wins against all bases.**
- Constraints follow the same rule.
- Inheritance is structural. It is not polymorphic.
- Errors: `inheritance-cycle` (an anchor that extends itself, directly or
  not directly), `invalid-base` (not an anchor), `unresolved-base`.

### 8.2 `super`, `self`, `this`

| Word | Meaning |
| --- | --- |
| `super` | The merged inheritance (the last base wins). It finds a property on an earlier base if the last base does not have it. If no base has it, the result is an error. |
| `self` | The most-derived anchor. It moves down: `${self.name}` in a base shows the name of the child when Piton compiles the child. |
| `this` | The anchor that holds the text. `${this.name}` in a base always shows the value of the base. In an override, `this` is the child. |

```piton
anchor Base:
    name: Base
    a: ${this.name}
anchor Mid extends Base:
    name: Mid
    a: over ${this.name}
    b: ${self.name}
anchor Final extends Mid:
    name: Final
```

`Final` → `{"name": "Final", "a": "over Mid", "b": "Final"}`.

- `self` moves only through inheritance. When you read `${Other.note}`, Piton
  calculates it with `self` = `Other`.
- `this` and `self` always mean an anchor. This is also true in a nested
  dictionary.
- The compiler reports `this`, `self` or `super` outside an anchor:
  `invalid-this`, `invalid-self`, `invalid-super`.

### 8.3 Abstract anchors

```piton
abstract anchor Construct:
    description:: string
abstract anchor Skill extends Construct as skill:
    useWhen:: string
skill Review:
    description: Reviews code
    useWhen: Asked for a review
```

- Piton never compiles or writes an abstract anchor.
- A concrete implementer must give all required properties of the full
  abstract chain.
- An abstract that has no implementer and no export gives a warning
  (`unimplemented-abstract`).
- An anchor can implement many abstracts. It can also extend concrete
  anchors. Many abstracts can share a base.
- If two abstracts put constraints on one property that cannot agree (string
  and number), the result is `conflicting-abstracts`. Constraints that
  overlap resolve with "the right side wins".
- Constraints from concrete bases follow usual inheritance. The right side
  wins and there is no error.
- Export abstracts with a keyword. The anchor of the keyword goes last and
  wins.
- Type constraints:
  - `A[]` matches only anchors that implement `A` **directly**. With many
    abstracts, Piton uses the abstract that wins: the abstract of the
    keyword, else the abstract at the right.
  - `extends A[]` matches each concrete anchor with `A` in its chain.
- Piton keeps properties that are not in the shape of the abstract. Belay
  writes them.

### 8.4 Keywords

```piton
anchor MyAnchor as my-anchor:
    description: Mine
my-anchor Child extends Other:
    description: ${super.description} more
```

- `my-anchor Child:` is equal to `anchor Child extends MyAnchor:`.
- `my-anchor Child extends Other:` is equal to `extends Other, MyAnchor`.
  **The anchor of the keyword is last and wins.**
- An anchor has one keyword only.
- You can build keywords on keywords:
  `export abstract operator ArithmeticOperator as arithmetic-operator:`.
- The file must `use` the module that exports the keyword.
- A file can declare a local keyword with the name of an imported keyword.
  Example: `abstract anchor MyTest extends Test as test:` gives each local
  `test` the tags of `MyTest`.

## 9. Compilation

- **`piton compile <file>`** gives one object with one key for each export.
  It does not show names without an export, or abstracts. Spec JSON examples
  do not show this wrapper.
- **`piton build`** compiles all exports of the entry and all items that
  they refer to. It compiles nothing more. It compiles exports that nothing
  uses. It never compiles abstracts.
- Renderers: `json` (default), `yaml`, `markdown`. Belay builds on the
  Markdown renderer. Plugins get the renderers from the host (11.5).

**Markdown serialization** (`spec/scope/belay/Compilation.pi`):

- Anchor names and property names become title headings:
  `myProperty` → `My Property`. Piton does not split a run of capitals:
  `whatIsAType` → `What Is AType`.
- `${MyAnchor}` gives the source name as written. Piton does not change it to
  title case.
- The depth of a heading follows the nesting. Below level 6, Piton writes
  bold labels (`**G**`). Sibling keys get the same heading level.
- **Each anchor property gets a heading**, also `false` or `42`.
- A **pure dictionary** has only simple values or pure dictionaries. It has
  no lists and no mixed content at any depth.
- Piton writes a pure dictionary as it is, in a code fence. Its keys are not
  headings. This is also true when it is in mixed content.
- **Strings are simple, also long prose.** To get a heading for each key,
  start the block with a prose line or add a list.
- Dictionaries that are not pure, and mixed content, get a heading for each
  key.
- Lists become nested Markdown lists. Mixed lists keep their order. A
  primitive value is its text.
- `{Other}` is a copy. Only `@{Other}` makes a link.
- Intrinsic values, strings and references stay different. Piton never gets
  the identity of a reference from a heading.
- Target frontmatter and body layout go around the serialized content.

## 10. `piton.config.pi`

Piton reads the nearest `piton.config.pi` in the current directory or a
directory above it. Its anchors come from the bundled `@piton/config`. It
exports its `piton-config` anchor. The LSP treats it as config.

```piton
use @piton/config
use @piton/belay
use @piton/packaging
use @piton/test
from @piton/belay import ClaudeCodeAdapter

export piton-config Config:
    root: ./spec
    entry: ./spec/index.pi
    output: ./dist
    renderer: json
    packages:
        - {MyPackage}
    dependencies:
        - https://github.com/org/repo
            tag: 1.0
    plugins:
        - {BelayConfiguration}
        - {Tests}

belay-config BelayConfiguration:
    codeRoot: ./src
    shapeRoot: ./spec/shape
    adapters:
        - {ClaudeCodeAdapter}

piton-package MyPackage:
    name: my-package
    root: ./spec/pkg

test-config Tests:
    agent: codex
```

| Field | Meaning |
| --- | --- |
| `root` | Required. The source root. Imports that start with `/` resolve here. |
| `entry` | The default is `index.pi` in `root`. If that file is not there, the result is an error. |
| `output` | The build output. The default is `./dist`. |
| `renderer` | `json` (default), `yaml`, `markdown`. |
| `packages` | The packages that this repository **offers** (13). |
| `dependencies` | The packages that this project installs (13). |
| `plugins` | Plugin declarations that the project adds, and settings anchors for its plugins (11.6). Piton warns about an item that is not an anchor and ignores it. |

- **The `frameworks` key is removed.** A config that has it gives
  `frameworks-removed`. Move each item to `plugins:`, or run
  `piton migrate` (11.12).
- A framework is a plugin with a build. **A `belay-config` under `plugins:`
  starts the Belay output.**
- If a plugin builds output, Piton writes renderer output only if you set
  `output` or `renderer`.
- In `PitonConfig`, all optional fields have the form `:: <type>:: null: null`.
- `PitonPackage` (`@piton/packaging`) has `name` (optional), `root`
  (required) and `dependencies` (optional).
- You can use the keywords and modules of a bundled plugin without
  registration. Each file must still `use` or import what it needs.

Config diagnostics: `missing-config`, `unknown-config-key`,
`frameworks-removed`, `package-without-root`, `invalid-package-name`,
`multiple-pins`. Belay settings diagnostics: `unknown-adapter`,
`invalid-adapter`, `invalid-cross-discovery`, `invalid-instruction-byte-limit`.

## 11. Plugins (`spec/scope/plugins/`)

Plugins add CLI commands, build output, build diagnostics, the anchors that
they reach, and Piton files that you can import. They do not change the
compiler.

- You declare a plugin in Piton and write it in TypeScript.
- oxc transpiles the code. Embedded QuickJS (rquickjs) runs it.
- You install nothing. Plugins need no Node and no build step.
- **Compilation stays pure.** `piton compile` and the LSP while you type read
  only Piton. Plugin code runs only for its command, for build plans of
  `piton build` and `piton check`, for reach hooks of `piton reach`, for
  inspect hooks before a command (11.10), and when the editor saves a file.
- **A plugin can do only what it declares.** It can always read the compiled
  project.
- Bundled plugins and installed plugins work the same. The binary holds the
  files of a bundled plugin.
- Three plugins are bundled: Belay (`plugins/belay`), the test plugin
  (`plugins/test`) and the migrate plugin (`plugins/migrate`). Each has
  `plugin.pi` (its declaration), `src/` (its TypeScript) and, if it has
  Piton files, `pi/`.
- Not available now: renderers for `piton compile`, diagnostics outside a
  build, and extensions of a compiler command.

### 11.1 Declaration (`@piton/plugin`)

```piton
use @piton/plugin

export piton-plugin TestPlugin:
    name: test
    entry: ./src/index.ts
    settings: TestConfig
    requires: [belay]
    description: Has an agentic tool judge the project's tests.
    capabilities:
        process: [claude, codex, opencode, muse]
        temporaryFiles: true
        interactive: true
    commands:
        test:
            about: Have an agent judge the project's tests
            arguments:
                targets:
                    help: Tests to run, by name, file or directory
                    positional: true
                    multiple: true
                jobs:
                    short: j
                    type: number
                    minimum: 1
                serial:
                    type: flag
                    conflicts: [jobs]
```

| Field | Rules |
| --- | --- |
| `name:: string` | Required. Lowercase words with hyphens. The Piton files of a bundled plugin are `@piton/<name>`. If two plugins have the same name, Piton reports the second and does not load it. |
| `entry:: string` | Required. A TS or JS module relative to the declaring file. It must be in that directory and it must exist. Its default export is `definePlugin(...)`. |
| `description` | Optional string. `piton plugins` shows it. |
| `settings` | Optional string. The name of an abstract anchor that the plugin exports. A settings anchor of a project implements it. |
| `commands` | Optional dictionary. See below. |
| `capabilities` | Optional dictionary (11.2). Without it, the plugin can only read the project. |
| `requires:: string[]` | Optional. The names of plugins that this plugin uses (11.4). |
| `placeholders:: string[]` | Optional. Exported names whose value Piton knows only when it writes the output (11.5). |
| `build:: boolean` | Optional. `true` if the plugin has a build hook (11.3). Without it, `piton build` does not run the plugin. |
| `inspect:: boolean` | Optional. `true` if the plugin has an inspect hook (11.10). |
| `reach:: boolean` | Optional. `true` if the plugin has a reach hook (11.11). |

The host reads `build`, `inspect` and `reach` to know which plugins to ask.
It does not run the plugin to know this.

Commands:

- The key is the command name. It is lowercase with hyphens and comes after
  `piton`.
- `about` is one line. It shows in `piton --help` and at the top of the
  command help.
- `group` is optional. `slice` in the group `belay` is `piton belay slice`.
  `piton --help` lists group commands below the group. The group is also a
  command that lists them.
- `help` gives more text. It shows in `piton <command> --help`.
- `arguments` is an ordered dictionary.
- If a command has the same name as a command of another plugin or a
  built-in command, Piton reports it and leaves it out. The same rule
  applies in a group. A group cannot have the name of a command.

Argument settings are all optional:

| Setting | Meaning |
| --- | --- |
| `help` | The help text. |
| `positional` | `true` for an argument without an option name. |
| `multiple` | `true` if you can give it more than one time or with many values. |
| `type` | `string` (default), `number` or `flag`. |
| `choices` | The values that it accepts. |
| `short` | One letter, for example `-j`. |
| `minimum` | The lowest number. |
| `conflicts` | Arguments that you cannot give with it. |
| `default` | The value when you do not give it. |
| `trailing` | For the last positional `multiple` argument. `true` takes all that follows as it is, also values like options (`piton agent claude --resume`). |
| `optionalValue` | For an option that takes one value. `true` lets you give the option with no value (`piton test --model`). |

- The key is the long option.
- A positional argument cannot have the type `flag` or a short name.
- Only an option that takes one value can have `optionalValue`. A
  positional, `multiple` or `flag` argument cannot have it.
- An unknown setting makes the declaration not valid. A `conflicts` value
  that names an argument that is not there also makes it not valid.
- Values come to the handler by name: a flag gives a boolean, a number gives
  a number, `multiple` gives a list, and an absent value gives null.
- An `optionalValue` option without a value gives an empty string. An empty
  string is different from null.
- clap checks values first (choices, minimum), as for built-in commands.
- A declaration with a bad form gives a warning that names the file. Piton
  skips only that plugin.

### 11.2 Capabilities

A plugin can always read the compiled project. It must list all other
operations. An unknown capability makes the declaration not valid.

```piton
capabilities:
    process: [claude, codex]
    files: read
    temporaryFiles: true
    outputs: [.claude/**, **/CLAUDE.md]
    interactive: true
    edits: [piton.config.pi]
```

| Capability | Meaning |
| --- | --- |
| `process: [names]` | The programs that the plugin can run and find on PATH. The names must be exact. |
| `files: read \| none` | The default is `none`. `read` lets the plugin read files and list directories in the project, never outside it. |
| `outputs: [globs]` | The paths that the build of the plugin can write, relative to the project root. `*` matches in one directory. `**` matches across directories. Do not put quotes around them. |
| `edits: [globs]` | The project files that the plugin can propose to change, relative to the project root, as for `outputs`. Piton applies an edit only when the user agrees, or when the user runs the command of the plugin. An edit outside the globs is an error, and Piton offers nothing of it. |
| `interactive: true` | The plugin can show a menu on the terminal. The menu shows only when stdin and stdout are terminals. |
| `temporaryFiles: true` | Temporary files outside the project. Piton removes them when the command ends. |

- **Plugins never write project files themselves.** A build returns a plan,
  and the host writes it inside `outputs`. An inspect hook returns proposed
  edits, and the host writes them inside `edits` if the user agrees.
- A plugin with a build and no `outputs` can only report diagnostics.
- A refused operation throws an error, for example: "plugin `hello` may not
  run `ls`: its declaration does not list it under capabilities.process".
  The plugin can catch it. If it does not catch it, the command fails.
- Capabilities do not put the programs that a plugin runs in a sandbox.
- `piton plugins` shows the capabilities of each plugin.

### 11.3 Build

A build hook returns a plan. The host checks the plan and writes it. The same
plan serves `piton build`, `--dry-run`, `piton check` and the editor.

```typescript
export default definePlugin({
  build(ctx) {
    const compiled = ctx.compile();
    return {
      files: [{ path: "docs/index.md", contents: "# Index\n", origin: "Index" }],
      copies: [],
      diagnostics: [],
    };
  },
});
```

| Plan field | Content |
| --- | --- |
| `files` | Each file: its path relative to the project root, its full contents, its source anchors and its source files. |
| `copies` | Each file to copy without a change, from a file that the plugin can read. Piton keeps the bytes and the execute bit. |
| `diagnostics` | Each problem: severity, code, message and source location. |
| `locations` | Optional. The file and heading of each anchor. Slice commands use them. |
| `targets` | Optional. Labels for the manifest, for example the documentation date of each target. |

Rules for the host:

- It writes a plan only if the plan has no errors from the host or the
  plugin. It checks the full plan before it writes.
- Each path must be in the project and in the declared `outputs`.
- Two files (or a file and a copy) with one path must have the same content.
- Two plugins cannot write one path.
- The source of a copy must be a file that the plugin can read.
- Each problem is an error that names the path and the plugin. Then Piton
  writes nothing.
- Piton writes each file through a staging file. A stopped build never
  leaves half of a file.
- `.piton/manifest.json` records each written path, in path order, with the
  `targets` of the plans.
- The next build removes the paths that the last build wrote and this build
  does not write. It removes nothing more. A file in the manifest that a user
  edited still belongs to the build.
- The same sources, configuration and toolchain give the same bytes. The
  host sorts nothing that a plugin ordered. Plan in a stable order.
- If plugins build output, it replaces the renderer output. If you set
  `output` or `renderer`, Piton writes both.
- A bundled plugin with a build builds only for a project that lists its
  settings. A project without Belay settings gets no agent files.
- `piton build --dry-run` runs each build hook, checks the plans, reports
  what it will write and remove, and writes nothing.
- `piton check` runs each build hook and reports the plan diagnostics. It
  writes nothing.
- The LSP runs build hooks when you save a file, never while you type.

### 11.4 Dependencies

```piton
piton-plugin TestPlugin:
    name: test
    requires: [belay]
```

```typescript
// In Belay:
export default definePlugin({
  exports: { agents: (ctx) => ({ list, launch, fluency }) },
});

// In the test plugin:
const agents = ctx.require("belay").agents;
```

- Piton loads the required plugins into the same runtime, before the plugin
  runs.
- Each plugin has its own modules. A plugin reaches another plugin only
  through `ctx.require`, with a name that its declaration lists.
- These are errors in the declaration, and Piton does not load the plugin:
  a requirement that the project does not have, a requirement on itself,
  and a circle of requirements.
- `definePlugin` takes `exports` next to `commands` and `build`.
- An exported factory function gets the context of the exporting plugin. It
  runs with the capabilities of that plugin, never with the capabilities of
  the caller.
- A requirement gives access only to exports. The commands and the build of
  a plugin run one time, also when many plugins require it.

### 11.5 Rendering

The host gives its renderers to plugins. Plugin output is then the same as
the output of `piton compile` and `piton slice`.

- **Link space.** A plugin describes its documents: each output path with its
  anchors in order. It also names each anchor that has a file of its own and
  no headings (for example a `SKILL.md`). The host makes a link space from
  this.
- In a link space, the host renders a document, sections or inline text for
  a given path. References become links relative to that path.
- A link to an anchor goes to its heading. A link to a property goes to the
  heading of the property. Repeated headings get different fragments, as a
  Markdown viewer numbers them.
- A reference to an anchor that nothing wrote becomes its name, not a link.
- **Placeholders.** Piton evaluates each placeholder to a marker that cannot
  match prose. The plugin replaces the marker in each file that it plans.
  The host gives the markers and a function that makes paths relative to the
  file.
- **Encoders.** Real encoders write YAML frontmatter, TOML strings and JSON
  strings.
- **Paths.** The host writes paths with forward slashes on all platforms.

### 11.6 Sources and settings

- A project adds a plugin when it lists the `piton-plugin` anchor under
  `plugins:`. Piton reads the code from the directory of the declaring file.

```piton
use @piton/config
use @piton/plugin

export piton-config App:
    root: ./spec
    plugins:
        - {Hello}

piton-plugin Hello:
    name: hello
    entry: ./plugin/index.ts
    commands:
        hello:
            about: Say hello
```

- To share a plugin, put it in a package. The project imports the
  declaration from the package and lists it under `plugins:`. Import the
  Piton files of the plugin by the package name.
- Settings: an anchor that implements the settings abstract of the plugin.
  List it under `plugins:`. For bundled plugins it must come from the package
  of the plugin (`belay-config`, `test-config`).
- A plugin has one settings anchor at most. Two settings anchors give an
  error when the plugin reads them.
- The plugin validates the values itself. It reports a bad value where you
  wrote it.
- Each anchor under `plugins:` is a declaration or settings. Piton warns
  about other items.
- Piton finds plugins on each `piton` run, from the nearest
  `piton.config.pi`. This reads only Piton. Plugin commands work from all
  directories of the project.
- The config must have no errors before a plugin command runs. Plugin
  problems are warnings.
- Exception: the commands of a plugin with an inspect hook run also when the
  config has errors. Their work can be to repair the config.

### 11.7 Runtime

- Piton transpiles each module when the plugin imports it. It removes types
  and does no lowering. `.js` and `.mjs` files stay as they are.
- As in TypeScript, Piton removes an import that nothing uses as a value.
- A syntax error gives the file and the line.
- A plugin can import `@piton/plugin` and relative files. Piton tries the
  path as it is, then `.ts`, `.js`, `/index.ts`. A plugin can never import a
  file outside its directory.
- Each command run gets one runtime. Nothing stays from one command to the
  next.
- The event loop has one thread. The command ends when the promise of the
  handler settles.
- If the plugin waits for something that cannot answer, the command fails.
  It does not hang.
- At exit, Piton stops all processes that the plugin started and removes the
  temporary files.
- A throw or a load failure fails the command. Piton shows the plugin name,
  the error and the JS stack, and exits with 1.

### 11.8 API

The API is in `crates/piton-plugin/api/index.ts`. For editors, a
`tsconfig.json` maps `@piton/plugin` to it, with strict checks and no emit
(`plugins/test/tsconfig.json`). Piton never reads that file.

```typescript
import { definePlugin, type Context, type Json } from "@piton/plugin";

async function hello(ctx: Context, args: Record<string, Json>) {
  const compiled = ctx.compile({ workspace: true });
  for (const anchor of compiled.anchors({ exported: true, withinSourceRoot: true })) {
    ctx.stdout(`${anchor.name}\n`);
  }
  ctx.report.done("listed the anchors");
  return 0;
}

export default definePlugin({ commands: { hello } });
```

A handler returns the exit code. If it returns nothing, the code is 0.

`definePlugin` also takes hooks. The declaration must declare each hook.

| Hook | Returns |
| --- | --- |
| `build` | The build plan (11.3). Needs `build: true`. |
| `inspect` | The findings: what is out of date (11.10). Needs `inspect: true`. |
| `reach` | The anchors that the plugin reaches (11.11). Needs `reach: true`. |
| `exports` | What plugins that require this plugin can use (11.4). |

| Context member | Meaning |
| --- | --- |
| `plugin` | The plugin name. |
| `terminal` | Are stdin, stdout and stderr terminals, the width, and colour (`NO_COLOR`). |
| `project` | Absolute paths: root, source root, entry, output, config path. With Belay settings: code root, shape root and adapters. |
| `settings` | The settings anchor, or null. It includes the properties of each anchor that it names (for example the adapters) and their locations. |
| `compile({workspace})` | Compile the project. `workspace` compiles all sources below the root. |
| `spawn` | Run a permitted program with args, an optional cwd in the project, and env. It runs in the project root with an empty stdin. |
| `exec` | Run a permitted program with the terminal. The user does the input. It returns the exit code. |
| `applyEdits` | Write proposed edits (a project file and its full new contents) for a command that the user ran to make them. Each path must be in the `edits` capability. It returns the changed paths. |
| `choose` | Show a menu and return the selected item, or nothing on cancel or without a terminal. Needs `interactive`. |
| `resource` | The text of a file in the directory of the plugin. It needs no capability. |
| `onPath` | Is a permitted program on PATH. |
| `readFile` | The text of a file in the project. Needs `files: read`. |
| `stat` | File, directory or other, size, execute bit, or null. It follows symlinks. Needs `files: read`. |
| `temporaryFile`, `sleep` | Make a temporary file and return its path. Wait some milliseconds. |
| `paint` | Styles: error, warning, success, help, note, name, heading, dim. |
| `stdout`, `stderr` | Write text. They remove colour as the compiler does. |
| `stderrRaw` | Write to stderr without a change, for a status block. |
| `report` | error, warning, help and note messages. Piton keeps them until the end and shows a count. `done` and `stopped` write the outcome line. |
| `require(name)` | The exports of a required plugin (11.4). |

The compilation:

- `anchors({extends: {module, name}, exported, concrete, withinSourceRoot})`
  returns anchors in order of file, then name.
  - Each anchor has name, file, exported, abstract, keyword, resolved
    properties and location.
  - `extends` does not include the named anchor itself. A module that nothing
    loaded matches nothing.
  - `withinSourceRoot` does not include `tethers/` or bundled packages.
  - The location is where the anchor names the property itself, else where
    its name is.
- `exports`: the ids of the entry exports in export order, with forwarded
  exports.
- `values`: the resolved properties of an anchor as typed values, in order.
- `slot`: the declaration that gives a property its value. It tells the
  anchor, whether a value or only a type was given, and the location. It is
  null if the anchor has no such property.
- It can print a slice that cites a layout instead of the source, as
  `piton slice` does. It returns the exit code.
- `reportDiagnostics` shows the diagnostics of a compilation as `piton check`
  does. It tells if any were errors.

Property values as JSON:

- Strings are plain. References show as names.
- Implicit lists are lists.
- An anchor value is `{$anchor: name}`. A reference is `{$reference: name}`.

Typed values keep the differences that JSON loses. Each value is an object
with a `kind`:

| Kind | Content |
| --- | --- |
| null, boolean, number | The value. |
| text | The plain text and its references (anchor id and property path). |
| list | The items. |
| dictionary | The entries as key and value pairs, in order. |
| mixed block | The items. A key in the block is an entry. |
| anchor | The id and name. |
| reference | The id, path and written name. |

Other API rules:

- A located diagnostic renders like a compiler diagnostic. The plugin selects
  its code. The default code is the plugin name.
- Process output comes line by line, with the tag stdout or stderr. The
  stream ends after the process exits and all lines are read.
- If a signal stopped the process, the exit code is null.
- Await many processes together to run them in parallel.

### 11.9 `piton plugins`

- It lists bundled plugins, then declared plugins.
- For each plugin it shows: name, origin, description, commands, settings
  anchor, package and capabilities. If the plugin has no capabilities, it
  tells you that the plugin can only read the project.
- Problems come before the outcome line: config diagnostics, bad
  declarations, and anchors under `plugins:` that are not a plugin or
  settings.
- The outcome line gives the count of plugins. The exit code is 0.

### 11.10 Inspection (`spec/scope/plugins/Inspection.pi`)

A plugin can examine a project before a command runs, and offer to change
it. An example is to update a project for an older Piton.

```typescript
export default definePlugin({
  inspect(ctx) {
    return {
      findings: [{
        message: "piton.config.pi lists frameworks, which is now plugins",
        edits: [{ path: "piton.config.pi", contents: "..." }],
      }],
    };
  },
});
```

When:

- The host runs each inspect hook before each command, in a project with a
  `piton.config.pi`.
- It does not run them before `piton lsp`, `piton --help`, `--version`, or
  the command of the plugin that inspects.
- A hook runs before the project must compile. Thus it can help a project
  that does not compile. It reads files directly, and it must be quick.

Findings:

- Each finding is a message and the edits that fix it. Each edit is a
  project file and its full new contents.
- A finding with no edits is only a report.
- Each edited path must be in the project and in the `edits` capability.
  Else the finding is an error of the plugin, and Piton offers nothing of it.

The question:

- If stdin and stderr are terminals, the host shows each finding with a diff
  on stderr. It asks one time for all findings: "Switch it over?". The
  default is yes.
- Yes: Piton writes each file through a staging file and reports each
  changed path. Then it runs the requested command again on the updated
  project.
- No: the project does not change, and the command runs as requested.
- Without a terminal, Piton asks nothing and changes nothing. It shows each
  finding as a warning that names the command that fixes it. Then the
  command runs.
- A hook that fails gives a warning, and the command runs. An inspection
  never stops a command.

### 11.11 Reach hooks (`spec/scope/plugins/Reach.pi`)

Some anchors are in use, but nothing in the specbase leads to them. An
example is a test. A plugin can name the anchors that it reaches itself.

```typescript
export default definePlugin({
  reach(ctx) {
    const compiled = ctx.compile({ workspace: true });
    const tests = compiled.anchors({
      extends: { module: "@piton/test", name: "Test" },
      exported: true,
      concrete: true,
      withinSourceRoot: true,
    });
    return { anchors: tests.map((test) => ({ file: test.file, name: test.name })) };
  },
});
```

- The hook returns anchors. Each anchor is its declaring file (as the anchor
  gives it, or relative to the project root) and its name.
- Piton ignores an anchor that names nothing declared.
- A file that nothing imports compiles with the project, so its anchors can
  be roots.
- Each returned anchor is a root next to the entry exports, at depth 0. What
  it imports, references, inherits or embeds is reachable through it.
- Such an anchor is not unreachable and not unused.
- `piton reach` without targets adds these roots. With targets, plugins add
  nothing.
- The LSP adds these roots to its unused check. It asks the plugins when you
  save a file, never while you type. It keeps the anchors until the next
  save.
- Reach hooks run only in a project with a `piton.config.pi`. A hook must be
  quick and change nothing.
- A hook that fails gives a warning. The other plugins still count. Reach
  never fails because of a plugin.

### 11.12 The migrate plugin (`spec/scope/migrate/`)

The bundled migrate plugin finds projects for an older Piton and offers to
update them. Its source is `plugins/migrate` (`plugin.pi` and `src/`). It
has no Piton files.

- Declaration: `inspect: true`, `files: read`,
  `edits: [piton.config.pi, **/*.pi]`. It runs no programs. It writes
  nothing itself.
- A migration finds one old form from the project files as they are,
  without a compile. It gives the full new contents of each file that it
  changes.
- When a change in Piton breaks existing projects, the change comes with a
  migration.

Migrations:

| Migration | Finds | Edit |
| --- | --- | --- |
| `FrameworksToPlugins` | A `frameworks` key in the `piton-config` anchor of `piton.config.pi`. | Without a `plugins` key, rename `frameworks` to `plugins`, and keep the items. With both keys, move the `frameworks` items to the end of `plugins`, and remove `frameworks`. An inline `plugins` list becomes a block. |

`piton migrate [--check]`:

- It applies each migration that the project needs, without a question. It
  reports each changed file.
- If the project needs no migration, it says that the project is up to date.
- `--check` changes nothing. It fails when the project needs a migration,
  and names each migration. Use it in CI.
- Exit code: 0 on success. With `--check`, 1 when a migration is necessary.

## 12. Belay

Belay is the bundled plugin `@piton/belay`. It is a vocabulary for agents.
It compiles into the files that agentic tools read.

- Piton does the parse, evaluation, modules, inheritance, reachability and
  rendering.
- Belay adds the four constructs, the `belay-config` settings anchor, the
  special imports, an adapter for each platform, its build, and two commands.
- The platform runs the result. A correct compile does not make a correct
  behaviour.
- The constructs and adapters are the Piton files in `plugins/belay/pi/`.
  The spec (`spec/scope/belay/`) describes them. A change to
  `plugins/belay/pi/Skill.pi` changes the framework.
- Piton assesses the implementation against the specification. Make lasting
  changes in the Piton source.

Belay plugin declaration (`plugins/belay/plugin.pi`):

| Item | Value |
| --- | --- |
| `settings` | `BelayConfig` (`belay-config`). |
| `build` | `true`. |
| `placeholders` | The five special imports (12.4). |
| `capabilities` | `files: read` (vendored files), `process: [claude, codex, opencode, muse]`, `temporaryFiles`, `interactive` (the model menu of `piton agent`), `outputs`: `.claude/**`, `.codex/**`, `.agents/**`, `.opencode/**`, `.muse/**`, `**/CLAUDE.md`, `**/AGENTS.md`. |
| `commands` | `agent` (15) and `slice` in the group `belay` (`piton belay slice`, 15). |

- The build of Belay is its plan (12.5). The host writes it and owns the
  manifest.
- The host Markdown renderer renders reference documents and construct
  Markdown in a link space of Belay. The host encoders write YAML and TOML.
- Belay exports `agents` for other plugins: `list` (the known tools, each
  with name, program and target), `fluency` (the prompt of `piton agent`)
  and `launch` (program, args and env to start a tool interactively, with an
  optional first message and an optional model). It also exports the models
  of each tool and the options that select a model. The test plugin uses
  them.
- The LSP shows Belay diagnostics and an output preview for each adapter
  after each save.

### 12.1 `belay-config`

```piton
export abstract anchor BelayConfig as belay-config:
    codeRoot:: string
    shapeRoot:: string:: null: null
    adapters:: list
    crossDiscovery:: string:: null: null
    instructionByteLimit:: number:: null: null
```

| Field | Meaning |
| --- | --- |
| `codeRoot` | Required. The location of the application code. |
| `shapeRoot` | Optional. The location of shape instructions. If you do not set it, `BELAY_SHAPE_ROOT` is the project root and Piton places nothing by shape. |
| `adapters` | Required. `ClaudeCodeAdapter`, `CodexAdapter`, `OpenCodeAdapter`, `MuseAdapter`. |
| `crossDiscovery` | Optional. `separate`: each tool is deployed alone, and each adapter writes a full tree. `allow`: accept that a tool shows the command skills of another tool as usual skills. If not set: one shared project (12.6). |
| `instructionByteLimit` | Optional number. Piton checks it for targets that have a limit (Codex `project_doc_max_bytes`). |

Paths are relative to the config file.

### 12.2 Constructs

All constructs extend the abstract `Construct`. It has `description` and
`prompt`, both `:: string:: null: null`. Piton keeps other properties and
writes them as Markdown sections after the prompt.

| Keyword | Extra property | Meaning and output |
| --- | --- | --- |
| `instruction` | none | Scoped guidance. **Its location is its meaning.** Piton maps it from `shapeRoot` to `codeRoot` (12.3). Instructions with the same scope go into one file for each target. Piton also keeps them in the shape reference tree. |
| `skill` | `useWhen`, `files` | The agent loads it when it needs it. The discovery description is `<description> Use when <useWhen>`. The body is the prompt, then the extra properties. |
| `command` | none | An explicit entry point. Its name is `x-<normalized name>`. It is native or translated for each adapter. |
| `agent` | `role`, `file` | Its name is kebab-case. The body starts with `You are a <role>`, then the prompt, then the extra properties. |

- In the Piton files, `useWhen`, `role`, `files` and `file` are
  `:: ... :: null: null`. Belay checks the requirements itself:
  - A skill needs `useWhen`, except a vendored skill.
  - An agent needs `role`, except a vendored agent.
- Piton checks required fields on concrete constructs after inheritance.
- Piton does not write a description or prompt that is not there. If a target
  needs a description, the result is `missing-description`. These need one:
  Claude Code agents; Codex skills, commands and agents; OpenCode skills and
  agents; Muse skills and commands.
- Piton writes the role and the skill description literally:
  - `role: careful reviewer` → `You are a careful reviewer`.
  - `Reviews code.` with `asked for a review` →
    `Reviews code. Use when asked for a review`.

```piton
use @piton/belay

export skill AnswerSpecQuestions:
    description: Reads the spec and answers questions about it.
    useWhen: asked how part of the specification works
    prompt: Read the spec under spec/ before answering.
    reportFormat: Save every report as a standalone HTML file.
```

`reportFormat` becomes the heading `# Report Format`.

**Native options** are metadata. They are never body text. The options are
`tools`, `model`, `allowed-tools`, `mode`, `permission`, `agent`,
`model_reasoning_effort`, `sandbox_mode`.

- An option that the target does not support gives `unsupported-option`.
- A bad value gives `invalid-option`.
- The types of these options are an open decision (12.8).

**Vendored skills.** The `files` property of a skill names a folder. Piton
copies the folder next to `SKILL.md`.

- Piton copies each regular file byte for byte. It keeps the relative path
  and the execute bit. It renders, links or changes nothing.
- The manifest records each file, so a later build can remove it.
- A symlink in the folder is an error. A path that is not a directory in the
  project is an error.
- If the folder holds a `SKILL.md`, that file *is* the skill. The anchor
  cannot set other properties (`vendored-conflict`).
- The `name` in that file must agree with the Belay name
  (`vendored-name-mismatch`). It must have a description where the target
  needs one.
- Without a `SKILL.md`, Piton renders the skill as usual and copies the
  folder next to it.

**Vendored agents.** The `file` property of an agent names its native
definition. Use this to keep native settings that Belay cannot carry to all
targets, such as the `tools` and `model` of Claude.

- The extension must be the one of the target agent format: `.md`, or
  `.toml` for Codex. Another extension is an error.
- Piton writes the file without a change.
- Each other property on the anchor is an error.
- If the agent format of the target holds a name, the name must agree with
  the Belay name.
- The file must have a description where the target needs one.

Rules for `files` and `file`:

- The value is one path, or a dictionary from target id to path.
- A key that is not a target id is an error.
- A built target that the dictionary does not name gives
  `missing-vendored-target`.
- `null` writes nothing for that target.
- A path resolves from the `.pi` file that declares it, or from the source
  root if it starts with `/`.
- If a target reads the copy of another target, Piton writes no copy. If
  `files` names a different folder for that target, Piton gives a warning.

With these rules, a tethered package can carry a skill from an upstream
project without conversion:

```piton
use @piton/belay

export skill Impeccable:
    files:
        claude-code: ./vendor/claude/impeccable
        codex: ./vendor/codex/impeccable
        opencode: ./vendor/opencode/impeccable
```

### 12.3 Shape mapping

- Piton takes the directory of an instruction relative to `shapeRoot`. It
  resolves that directory below `codeRoot`.
- If the directory is not there, Piton goes up to the nearest directory that
  exists. **It stops at `codeRoot`.**
- Examples:
  - `spec/shape/components/button/Button.pi` →
    `src/components/button/AGENTS.md`.
  - Two files in `input/` combine into one file.
  - `components/nonexistent/` → `src/components/AGENTS.md`.
- `AGENTS.md` here is only the generic name. Each adapter uses its own
  instruction file name.
- The file has a heading for each instruction (`# Button Shape`).
- The fallback changes only the location. The shape reference tree keeps the
  original path.
- Instructions outside `shapeRoot` are an open decision. The compiler gives
  an error (`instruction-placement-unspecified`).
- A location never makes sure that the tool loads the file at the start
  (12.6).

### 12.4 References and special imports

- Belay writes only what the build writes: the entry exports and the items
  that they refer to.
- Piton writes the anchors that it reaches to the `referenceRoot` of the
  adapter. It writes **one Markdown file for each source module**, at its
  path below `root`. Each anchor is a heading.
  Example: `spec/components/Button.pi` →
  `.claude/reference/components/Button.md`.
- This tree, with its `shape/` subdirectory, is not a discovery directory of
  a platform. An agent reaches it only through links.
- `@{X}` becomes a relative link with a fragment:
  `[Button](../../reference/components/Button.md#button)`. The link is lazy.
  It is never an `@import` of Claude.
- A reference to a skill, command or agent links to the output of that
  construct (for example its `SKILL.md`). Piton does not copy it.

**Special imports** come from `@piton/belay`. They are the placeholders of
the Belay plugin. In each file that Belay writes, a special import becomes a
path relative to that file, for each adapter. Use them with `${...}`. Piton
resolves them at compile time, not when the agent runs.

| Import | Value in a Belay file |
| --- | --- |
| `BELAY_COMPILED_SHAPE` | The compiled shape tree of the adapter (`.claude/reference/shape`). |
| `BELAY_AGENT_ROOT` | `.claude`, `.codex`, `.opencode` or `.muse`. |
| `BELAY_PROJECT_ROOT` | The directory of `piton.config.pi`. |
| `BELAY_SHAPE_ROOT` | `shapeRoot`, or the project root. |
| `BELAY_CODE_ROOT` | `codeRoot`. |

- Outside the Belay files (`piton compile`, renderer output of
  `piton build`):
  - `BELAY_PROJECT_ROOT` resolves the same way.
  - `BELAY_CODE_ROOT` and `BELAY_SHAPE_ROOT` become the project root.
  - `BELAY_AGENT_ROOT` and `BELAY_COMPILED_SHAPE` become their names.
- `prompt: Read ${BELAY_COMPILED_SHAPE} first.` in
  `.claude/skills/designer/SKILL.md` becomes `../../reference/shape`.
- You can combine them:
  `export FLUENCY_PROMPT_FILE: ${BELAY_PROJECT_ROOT}/FLUENCY_PROMPT.md`.

### 12.5 Build plan

The steps:

1. Take the validated constructs, after imports and inheritance.
2. Make one plan for each adapter.
3. Resolve locations and reference destinations.
4. Check shared paths and cross discovery across the full plan.
5. Serialize.
6. Validate links, metadata, names and ownership.
7. Give the plan to the host, which writes it (11.3).

### 12.6 Adapters

Each adapter extends the abstract `belay-agent-adapter`
(`plugins/belay/pi/adapters/Adapter.pi`). It has `description`,
`targetId`, `instructionFile` and `referenceRoot`. It also has
`documentationChecked`. The default is `2026-09-21`. For Muse it is
`2026-10-13` (Muse Code 1.4.7). The manifest records it in `targets`.

| | Claude Code (`claude-code`) | Codex (`codex`) | OpenCode (`opencode`) | Muse (`muse`) |
| --- | --- | --- | --- | --- |
| Instructions | `CLAUDE.md` | `AGENTS.md` | `AGENTS.md` | `AGENTS.md` |
| Reference root | `.claude/reference` | `.codex/reference` | `.opencode/reference` | `.muse/reference` |
| Skill | `.claude/skills/<n>/SKILL.md`. Frontmatter `name`, `description`. The directory is the name. | `.agents/skills/<n>/SKILL.md`, the same. | `.opencode/skills/<n>/SKILL.md`. Name: 1–64 lowercase letters and digits, single hyphens, the same as the directory. Description: 1–1024 characters. | `.agents/skills/<n>/SKILL.md`. A description is always required. Never write `allowed-tools`. |
| Command | `.claude/skills/x-<n>/SKILL.md` with `disable-model-invocation: true`. Call it with `/x-<n>`. Never also write `.claude/commands/`. | `.agents/skills/x-<n>/SKILL.md` and `agents/openai.yaml` with `policy: allow_implicit_invocation: false`. Call it with `$x-<n>`. | Native `.opencode/commands/x-<n>.md`. Call it with `/x-<n>`. | Translated: `.agents/skills/x-<n>/SKILL.md` with `disable-model-invocation: true`. Call it with `/x-<n>`. |
| Agent | `.claude/agents/<n>.md`: `name`, `description`, optional `tools`, `model`. | `.codex/agents/<n>.toml`: `name`, `description`, `developer_instructions` (one TOML string: role, prompt, extras), optional `model`, `model_reasoning_effort`, `sandbox_mode`. | `.opencode/agents/<n>.md`: `description`, `mode` (`primary`, `subagent` or `all`; always explicit; default `subagent`), optional `model`, `permission`. The file name is the identity. It has no `name` field. | **Not supported.** No file. Warning `unsupported-construct`. Piton never changes it into a skill. |
| Native options | Agent: `tools`, `model`. Skill and command: `allowed-tools`, `model`. | Agent only: `model`, `model_reasoning_effort`, `sandbox_mode`. | Agent: `mode`, `model`, `permission`. Command: `agent`, `model`. | None. Approval mode, sandbox and permission profiles belong to Muse. |
| Load | Ancestor files at the start. Nested files when Claude reads that subtree. | The chain from the root to the current directory at the start. A same-directory `AGENTS.override.md` hides the file (`shadowed-instruction`). Byte limit (`instruction-over-budget`). | It searches up from the current directory. It prefers `AGENTS.md` to `CLAUDE.md`. Do not assume that it loads nested files (`unguaranteed-instruction-scope`). It also finds `.claude/skills` and `.agents/skills`. | Rules files from the current directory up to the workspace root, only in trusted workspaces. `AGENTS.md` wins against a same-directory `CLAUDE.md`, which it ignores with a warning. Descendant files are not sure to load. Skills come from `.agents/skills`, `.claude/skills`, `.codex/skills`, in that priority. It keeps one copy. It obeys the `disable-model-invocation` of Claude and the `openai.yaml` policy of Codex. |

Rules for all adapters:

- Piton diagnoses requested capabilities that a target does not support
  before it writes output.
- Each behaviour is native, translated or not supported. If Piton cannot show
  a required behaviour, it fails with a diagnostic that tells you what to do.
- Never change an agent into a skill without a diagnostic. Never change a
  command into a skill that the model can start.
- Piton writes model and permission settings only if you configure them. It
  does not write settings that are not configured.
- Piton validates options against the platform. Prose never goes into
  metadata. Target settings stay apart from extra prose properties.
- **A prompt that asks for restraint does not enforce it.** Claim enforcement
  only for platform controls. A translation never gives more access.
- The `allowed-tools` and the subagent `tools` of Claude are different.
  Neither is a portable sandbox.
- A restriction that Piton must enforce but cannot show is a diagnostic.
- Piton normalizes names before it makes the plan. Commands keep `x-`.
- Collisions give `name-collision` or `output-collision`. Two items with the
  same content, references and activation are not a collision.
- Generated links are relative. They must point to planned outputs
  (`broken-reference-link`). Piton does not check Markdown links that you
  write by hand.
- Piton writes metadata and body one time. A real YAML or TOML encoder writes
  frontmatter. Piton checks file names and metadata against the target schema
  before it writes.

**Many adapters share one project**, because the tools read the files of each
other:

- The first adapter in the list owns the single reference tree, with the
  compiled shape. All adapters link into it. The other adapters write no
  reference tree.
- With one adapter, or with `separate`, each adapter owns its own tree.
- The first adapter in the list writes a shared instruction path
  (`AGENTS.md`) one time. Muse shares the `AGENTS.md` of Codex and OpenCode.
- If a tool finds the skill directory of another enabled adapter, its adapter
  writes no copy of a skill that the other adapter writes there. It links to
  that skill. OpenCode and Muse do this for Claude Code and Codex skills.
  Muse also does it for commands, because it obeys their activation metadata.
- A construct that a target does not support (a Muse agent) links to the
  artifact of another adapter. If there is none, it links to its reference
  document.
- OpenCode finds the `x-` command skills of other adapters. This gives
  `cross-discovery-activation`. Two settings stop the error:
  - `opencode.json` or `opencode.jsonc` denies them:
    `"permission": {"skill": {"x-*": "deny"}}`. The last pattern that matches
    wins. This rule does not change the native `x-` commands of OpenCode.
  - `crossDiscovery` is set.
- A tool that finds one skill in more than one directory gives a
  `cross-target-discovery` warning. This does not apply if the tool keeps
  only one copy (Muse).
- With `crossDiscovery: separate`, each adapter writes a full tree. Writes to
  one path that do not agree give `output-collision`. A shared instruction
  file is not a conflict.

Rules for each adapter:

- **Claude Code**:
  - The agent identity comes from the normalized name, never from a heading.
  - Never copy `tools` or `allowed-tools` to another target without a mapping.
  - Claude Code still supports `.claude/commands/`. Belay writes commands as
    skills.
  - Nested instructions keep their mapped scope and links relative to the
    target.
- **Codex**:
  - Do not write `AGENTS.override.md` to win precedence. Diagnose a
    same-directory `AGENTS.override.md`.
  - Codex does not have to load guidance below the start directory.
  - The limit is `instructionByteLimit`.
  - Do not write deprecated custom prompts. Do not make a `.codex/commands/`
    directory.
  - Never map generic `tools`.
  - Use only the standalone format for custom agents.
  - Guidance cannot change the sandbox or the approval policy.
  - Piton diagnoses tool and model settings on a command.
  - A command must make both of its files.
  - The agent TOML must parse and have its three fields.
- **OpenCode**:
  - Do not assume nested loads or chains.
  - Give more guidance only through explicit `instructions` config, never
    global config.
  - Shape scope that is required but that OpenCode cannot make sure of is an
    error.
  - Frontmatter that OpenCode does not know enforces nothing.
  - `model` includes the provider.
  - Keep command argument placeholders.
  - OpenCode never enforces `allowed-tools` on commands.
  - Agents use `permission`, not the deprecated `tools`. Map them only when
    the meaning is the same.
  - Refuse conversions that are not clear.
  - Keep the location, but do not claim that activation is the same in all
    tools.
- **Muse**:
  - Commands keep `x-` and `disable-model-invocation`.
  - Piton reports each agent as not supported. Muse takes agents only from
    plugins or from an overlay for one run. A build can write neither.
  - Piton still writes the reference tree.

Other diagnostics: `invalid-artifact-name`, `missing-command-prefix`,
`description-too-long`, `output-outside-project`, `unowned-output`,
`duplicated-content`, `unrepresentable-reference`, `missing-reference-target`,
`unplaceable-instruction`, `invalid-vendored-path`, `unknown-vendored-target`,
`missing-vendored-files`, `unsupported-vendored-file`.

### 12.7 Guarantees

- The same source, configuration, directory state and toolchain give the
  same bytes.
- Combined instructions have a stable order.
- Piton quotes and escapes frontmatter correctly for each format.
- Piton reports reference targets that are not there and metadata that a
  target does not support. It does not hide them.
- `.piton/manifest.json` records generated files and the documentation date
  of each target. Thus a cleanup never deletes user files.
- A failure names the source anchor and property.
- Output stays in its boundaries.

### 12.8 Open decisions (`spec/scope/belay/Decisions.pi`)

If your work needs one of these decisions, say "unspecified". Do not guess.
Tell the user about it.

- Instructions outside `shapeRoot`.
- The types of `tools`, `allowed-tools` and `model`, and their mapping for
  each adapter.
- Names: normalization of skill file names and command names, the split of
  acronyms, and collisions.
- Workflows: are they always compositions of the four constructs? Do not add
  a `workflow` keyword.
- How Piton writes options that are specific to a platform
  (`Adapter.pi`, `metadata`).

## 13. Packages

A package is a git repository or a local directory. Piton **vendors** it
into the project, and you commit it. Piton fetches nothing at compile time.

Offer packages:

- List `piton-package` anchors under `packages:` in the config. You can
  declare them in any file.
- `root` is relative to the config.
- One repository can offer many packages. Each has its own root and
  dependencies.
- **A project never installs its own packages.**

Names:

- The name is `name` (it can be kebab-case), else the anchor name.
- Names are flat and have no `/`.
- `@piton/...` is reserved for bundled packages.

Dependencies:

- Each dependency is a source. It can have one indented `commit:`, `tag:` or
  `branch:`.
- More than one pin gives `multiple-pins`. The compiler also gives
  `dangling-pin` and `unknown-pin`.
- With no pin, Piton takes the head of the default branch.
- Pins are strings: `tag: 1.0` → `"1.0"`.
- To select some packages of a repository, write `packages: [ui-kit]` below
  the source. Without it, Piton takes all packages.

Local sources:

- A local source is an absolute path, `~/...`, or a relative path. Piton reads
  a relative path from the directory of the `piton.config.pi` that lists it.
- A value with a scheme (`https://`, `file://`) or `git@host:path` is a URL.
- Piton clones a git directory like a URL. You can pin it, and the lock
  records its commit. Piton takes only committed content. It warns about
  changes that are not committed.
- Piton copies a plain directory as it is now. A pin on it is an error. The
  lock records no commit. `update` takes its current content.
- You cannot tether a directory that holds the project.
- A package from a local directory can list relative path dependencies. A
  package from a URL cannot.

Rules:

- Project dependencies and package dependencies are separate. **Packages do
  not nest.**
- If versions conflict, the newest commit wins, and Piton gives a warning.
- **Install**: Piton installs into `tethers/<name>` next to the config. The
  directory holds the files of the package `root`. Piton removes all git
  traces.
- A repository that offers no packages installs complete, below the name
  that its URL gives.
- **Atomic**: Piton resolves and checks all packages before it writes
  anything. If one package fails or conflicts, Piton places none. The config,
  `tethers/` and the lock do not change.
- Piton adds a source only if it can also fetch all that the source requires.
- **Two packages cannot share a name.** This applies to a name that another
  source already tethered, a name that the project offers, and a name
  published twice in one install. The result is an error, never a
  replacement. Use `--as` to give a new name.
- **Import** by name: `from my-package import X`, `use my-package`,
  `use my-package/X`. The package name is a placeholder for a path. If the
  package is not there, the result is an error.
- **Lock**: `.piton/tether.lock` is next to `piton.config.pi`. It holds the
  URL, commit and file hash of each package. `tether` and `update` write it.
  Commit it.
- **Edits**: each operation first compares the files with the lock. If you
  edited a package, the operation stops. It tells you to use `untether` or
  `update --force`.

Commands:

| Command | Behaviour |
| --- | --- |
| `piton tether [source] [--as name]` | Without a source: install all dependencies. With a URL or directory: install it, then add it to `dependencies`. Piton reads a relative path from the current directory and writes it relative to the config. |
| `piton update [packages...] [--force] [--diff]` | Install again. The default is all packages. |
| `piton untether <pkg> [--as name] [--no-rewrite]` | Move the package to `<root>/untethered/` and change the imports. It becomes project source. `--no-rewrite` moves the package and changes no imports. |
| `piton remove <pkg>` | Delete the package and remove it from `dependencies`. The command refuses if you edited the package or if a file imports it. |

`update` flags:

- `--force` discards your edits. It gives a warning for each package with
  the changes.
- `--diff` compares the content on disk before the update (with local edits)
  to the new content. It shows each changed region with 3 context lines.
- `--diff` shows old and new line numbers. An unchanged line shows both. A
  removed line shows the old number. An added line shows the new number.
- A separator line goes between regions. For each package, it shows how many
  files changed, or that none changed.
- If `update` refuses because of edits, it shows the edits. It fetches the
  package again at the locked commit and compares it with the disk.
- Piton calculates the diff itself. It does not need `diff`. On a terminal,
  it uses red and green.

## 14. Tests (`spec/scope/testing/`, bundled plugin)

A test is a plain English statement. An agent judges whether it is true or
false against the spec, the code or both. A test checks meaning. A compile
cannot check meaning.

The test plugin is in `plugins/test`. It is the reference for a plugin
author. It requires Belay for the tools, their models and the fluency
prompt. Its capabilities are the four tools, `temporaryFiles` and
`interactive`. It declares `reach: true`.

```piton
use @piton/test

export test SaveButtonSaves:
    expects: The Save button saves the document
    against: code
    tags: [editor, save]
```

`Test` (`test`) fields:

| Field | Rules |
| --- | --- |
| `expects:: string` | Required. The statement. |
| `against` | Optional: `spec`, `code` or `both`. |
| `description` | Optional. Context for the agent. It never changes the statement. |
| `tags:: string[]` | Optional. Piton trims tags and drops empty tags. Tags only select tests. Piton never sends them to the agent. |

Discovery:

- A test is each exported concrete anchor with the `Test` of `@piton/test`
  in its chain. The keyword or the chain is not important.
- Piton compiles all `.pi` files below the source root. The entry does not
  have to reach the tests.
- Piton skips tests without an export. Tests in `tethers/` do not belong to
  the project.
- The order is by file, then by name.
- The reach hook of the plugin returns each test (11.11). Thus `piton reach`
  and the LSP never report a test as unreachable or unused.

`TestConfig` (`test-config`, listed under `plugins:`):

| Field | Values | Default |
| --- | --- | --- |
| `against` | `spec`, `code`, `both` | `both` |
| `agent` | `claude`, `codex`, `opencode`, `muse` | `claude` |
| `model` | a model name, as the tool names it | the default of the tool |
| `jobs` | a whole number, 1 or more | `4` |

- Bad values give an error where you wrote them: `invalid-test-scope`,
  `unknown-test-agent`, `invalid-test-model` (a model that is not a name),
  `invalid-jobs`.
- A bad choice value (in settings, in a test or on the CLI) gives an error
  that names the correct choices. Nothing runs.
- Precedence:
  - `against`: CLI flag › test › settings › `both`.
  - `agent`, `model` and `jobs`: CLI flag › settings › `claude`, the default
    of the tool, or `4`. A test cannot set them.

### 14.1 `piton test`

`piton test [targets...] [--agent] [--model/-m [M]] [--against] [--jobs/-j N] [--serial] [--list] [--tags EXPR]`

Models:

- Without a model from the flag or the settings, the tool uses its default
  model. Before the tests run, a note lists the models of the tool and tells
  you to select one with `--model`. The run does not ask, because it is
  headless.
- `--model` with no value lists the models of the agent, one on each line,
  on stdout. It runs no tests and exits with 0. If the agent lists no
  models, the result is an error, and the exit code is 1.
- A `--model` value that is a model of the agent is that model. Another value
  is a search string, as for `piton agent` (15). Piton lists the matches, runs
  no tests and exits with 0.
- A model from the settings is never a search.
- The model list is the list of `piton agent`.

Targets:

- Piton first tries a target as an exact test name. Else it is a path to a
  file or directory, relative to the project root or absolute.
- Targets and tags combine with AND.
- An empty selection is an error. A target that matches nothing is an error.
- A project without tests succeeds with a report.
- `--serial` is the same as `--jobs 1`. `--jobs` with `--serial` is an
  error. `jobs` less than 1 is an error.

Tag expressions:

- Operators: `and`, `or`, `not`, parentheses. `&&`, `||` and `!` are the
  same.
- `not` before a term negates it. `not` between two terms means "and not":
  `editor not slow`.
- `not` binds most tightly. Then come `and` and infix `not`. Then comes `or`:
  `a or b and c` is `a or (b and c)`.
- Tags match exactly, with case. A test without tags matches only through
  `not`.
- A tag is a run of characters without spaces, parentheses, `!`, `&` or `|`.
  Examples: `needs-network`, `v1.2`. `and`, `or` and `not` cannot be tags.
- A parse error tells what Piton expected and where. Piton then compiles
  nothing.

The order of checks:

1. Config.
2. Flags.
3. Test settings.
4. Tag expression.
5. Compile and collect. Compile errors stop all work. Tests with a bad form
   (empty `expects`, bad `against`) also stop all work. Piton reports each by
   name with its file.
6. `--list` stops here.
7. The tool must be on PATH. Else the error tells you to use `--agent`.

What the agent reads:

| Scope | Content |
| --- | --- |
| `spec` | The `.pi` files below the source root, and the fluency prompt that `piton agent` gives the same tool. |
| `code` | The Belay `codeRoot`. Without Belay: the project root, without the source root, `tethers/`, the output and the generated agent directories. |
| `both` | The statement must be true in each. |

The session:

- Each test is one headless session in the project root, with an empty stdin.
  Piton asks for read-only access.
- The request gives the statement, description, name, file, scope, and the
  source root and code root relative to the project.
- The agent must not change, create or delete files. It must judge on
  evidence. A statement fails when evidence contradicts it or when there is
  no evidence.
- The agent ends with one verdict line:

```text
PITON-VERDICT: {"pass": true, "reason": "SaveButton's onClick calls saveDocument"}
PITON-VERDICT: {"pass": false, "reason": "No handler saves the document", "explanation": "The Save button has no click handler.", "fix": "Add a click handler to SaveButton in src/editor.ts. Make the handler call saveDocument."}
```

- `pass` is a boolean. `explanation` and `fix` are required when `pass` is
  false.
- The last valid verdict line counts. Piton accepts backticks around it. The
  request that the agent shows again does not count, because its example is
  not valid JSON. A revised verdict replaces the earlier verdict.
- For Claude, the answer is its message text and the final result. For other
  tools, it is stdout, then stderr.
- If the tool gives no reason, `reason` is empty.

Tests against `both`:

- Piton judges the spec and the code separately. The verdict also holds
  `spec` and `code`, each with `pass` and `reason`.
- With both results, the test passes only if both pass. The overall `pass`
  does not change this.
- A verdict without both results uses the overall `pass`.
- A test against `spec` or `code` alone ignores per-side results.

```text
PITON-VERDICT: {"pass": false, "reason": "The code has no save handler", "spec": {"pass": true, "reason": "The spec says Save saves"}, "code": {"pass": false, "reason": "No handler saves the document"}, "explanation": "...", "fix": "..."}
```

The explanation:

- The agent writes it on the verdict line in the same session. It costs no
  extra run.
- Piton asks for ASD-STE100 Simplified Technical English: 4 sentences or
  fewer, 20 words or fewer each, approved words, one word for one meaning,
  technical names as they are, one topic in each sentence, active voice,
  simple present, past or future tense, no -ing forms, no contractions, no
  phrasal verbs, articles, noun clusters of 3 words or fewer.
- It tells what is not there or different, and where.
- Piton shows it as written and does not check it. Piton keeps it only when
  `pass` is false. A failure without an explanation still fails and shows
  only the reason.

The fix:

- The agent also writes a `fix` in ASD-STE100, in the same session. It is a
  list of commands. It names each file and the change.
- Against `spec`, it changes the `.pi` source and not the code. Against
  `code`, it changes the code and not the spec. Against `both`, it has two
  parts, one for each side that fails.

The fix prompt:

- Each failed test gets a prompt that an agent with no other context can use.
- It holds: what to fix (the spec, the code or both), the statement, the test
  name and file, the scope, the reason, the per-side results for `both`, the
  explanation, and "What to change" (the `fix`).
- Without a `fix`, the prompt tells the agent to find the problem and change
  it.
- Its rules: change only the target scope (both sides must agree for `both`);
  never change the test; edit the spec only in `.pi` source, never in
  generated files; run `piton check` and `piton build` after a spec change;
  run `piton test <name> --against <scope>` to examine the fix.
- A test that Piton could not judge has no fix prompt.

```text
Fix the code so that this statement is true:

The Save button saves the document

The test `SaveButtonSaves` in `spec/tests/Editor.pi` examines the statement in the code (the files under `src`). It failed.
Reason: No handler saves the document
Explanation: The Save button has no click handler.

What to change:
Add a click handler to SaveButton in src/editor.ts. Make the handler call saveDocument.

Rules:
- Change only the code. Do not change the specification.
- Do not change the test.
- Do not edit generated files.
- To examine the fix, run `piton test SaveButtonSaves --against code`.
```

The hand-off:

- After the fix prompts and the outcome, a terminal menu lists `claude`,
  `codex`, `opencode`, `muse` and "No agent".
- The selected tool starts interactively in the project root. It gets the
  fluency prompt of `piton agent`, and the fix prompts as its first message.
- One prompt goes as it is. Many prompts go together under numbered
  headings, with a line that tells the agent to fix them one after the other.
- The menu shows only when stdin and stdout are terminals and a test failed.
  It does not show in a pipe, in CI, or if no test failed.
- A tool that is not on PATH gives an error. The exit code of the run does
  not change.
- After you select the tool, a second menu lists its models. "Default model"
  is first. Piton skips this menu if the selected tool judged the tests and
  had a model. The tool then keeps that model.
- If you cancel the model menu, nothing starts. A tool that lists no models
  gets a note and starts with its default model.

Errors:

- A nonzero exit or no verdict is an *error*. The test is not judged.
- The error shows the exit code and the last three lines of stderr (or
  stdout). It can also say that the session ended without a verdict line.

How Piton starts each tool:

| Tool | Command |
| --- | --- |
| Claude | `claude -p <request> --append-system-prompt <fluency> --allowedTools Read,Grep,Glob --disallowedTools Edit,Write,NotebookEdit,Bash --output-format stream-json --verbose` |
| Codex | `codex exec --sandbox read-only --skip-git-repo-check -c developer_instructions=<TOML string> <request>` |
| OpenCode | `opencode run --standalone <request>`, with `OPENCODE_CONFIG_CONTENT` that names a temporary fluency file and denies edit and bash |
| Muse | `muse exec -c run.developer_prompt=<JSON string> <request>`, in the sandbox of Muse |

With a model, each tool also gets its model option before the request, as
`piton agent` gives it.

Piton never claims more enforcement than the tool gives.

Output:

- Tests run in parallel, up to `jobs`. Piton shows each test when it ends.
  With `--serial`, it shows them in list order.
- Results go to stdout as blocks with blank lines between them.
- For each test:
  - `✓ pass`, `✗ FAIL` or `! error`, the name (shortened with an ellipsis if
    too long), and the duration `m:ss` at the right.
  - The file and tags, dimmed.
  - For `both` with per-side results: a `spec` line and a `code` line, each
    with its mark, word and reason.
  - The reason, aligned below the name.
  - For failures, the explanation below the reason, in the colour of the
    status.
  - Piton writes the reason, the explanation and each side reason as one
    line each. It does not wrap them, it does not indent continuation lines,
    and it writes no bar. The terminal wraps them. Thus you can copy a
    result as one coherent text.

```text
✗ FAIL   SaveButtonSaves                                  1:02
         spec/tests/Editor.pi · editor, save
         spec ✓ pass  The spec says Save saves
         code ✗ FAIL  No handler saves the document
         The code has no save handler
         The Save button has no click handler.
```

- The width is the terminal width of stdout, up to 100. When piped, it is 100.
- If a test did not pass, a "Not passing" recap follows. It has one line for
  each test: status, name and file. A failed `both` test also shows the
  sides, for example `(spec pass, code FAIL)`.
- Then the heading "Fix prompts" follows. Each fix prompt is under a rule
  with the test name, in list order. Piton does not wrap or indent it.
- The outcome goes to stderr: `✓ 9 tests passed in 4:12` or
  `✗ 9 tests: 7 passed, 1 failed, 1 could not be judged in 4:12`.
- On a terminal, a status block shows a spinner, counts, and for each test
  that runs, its time and last tool action. Results print above it. It
  disappears at the end.
- The last action of Claude reads like "reading `<path>`" or "searching
  `<dir>` for a pattern", with paths relative to the project. It also shows
  the first line of what Claude says. During its first turn, it shows
  "thinking". Other tools show their last printed line.
- If stderr is not a terminal, Piton writes a "running <name>" line when each
  test starts.
- `--list` shows tests in groups by file, with blank lines between files. For
  each test: the name, `against` and tags (dimmed), and the wrapped
  statement. The outcome is `✓ 9 tests in 3 files`. It runs nothing and needs
  no tool.
- Colour: green, red and yellow status, bold names, highlighted files, dim
  details and times. With `NO_COLOR` or a pipe, the text is plain.
- Exit code: 0 on success, 1 on failures or errors.

## 15. CLI

General rules for each command:

- It shows the results first, then a bold ✓ or ✗ outcome line.
- Then it shows diagnostics, with help and notes.
- Then it shows a count of errors and warnings. It does not show the count
  after a clean run, unless the command is a check.
- On a terminal: errors are red and warnings are yellow. Names that you type
  are highlighted. Depths, counts and provenance are dim. Paths have `+` and
  `-` marks, with a dim directory part.
- Colours and marks never carry information that the words do not carry.
- With a pipe or `NO_COLOR`, the text is plain. Bare results go to stdout.
- `piton --version`, and the version that the LSP reports, is major.minor
  from `Cargo.toml` with the commit count as the patch (for example
  `0.1.57`). `PITON_VERSION` overrides it. Without git, it is the
  `Cargo.toml` version.
- Plugin commands show in `--help` next to the built-in commands. They cannot
  replace built-in commands.
- `piton --update` replaces the running `piton` with the latest release.
  It is a flag of `piton`, not a command, and it takes no command. It is
  not `piton update`, which updates packages.
  - The source is the GitHub releases of `piton-lang/piton-rs`, or
    `PITON_DOWNLOAD_BASE` if it is set. This is the source of `install.sh`.
  - It finds the latest version from the redirect of `releases/latest`. It
    downloads the archive for this platform and architecture, and checks it
    against its `sha256` file.
  - It writes the new binary next to the old binary, then renames it over
    the old binary. Thus a failed update keeps the old binary. On Windows,
    it first moves the old binary to one side. On macOS, it clears the
    quarantine flag.
  - The outcome names the old and the new version. If `piton` is the latest
    release or newer, it replaces nothing and says so.
  - It downloads with `curl` or `wget`, and unpacks with `tar` or `unzip`.
    If none is installed, the error names what is missing.
- Each command is a `cli-command` anchor (`spec/scope/tooling/cli/index.pi`)
  with `commandName`, `positionalArguments` and `namedArguments`.

| Command | Source | Behaviour |
| --- | --- | --- |
| `check [paths...]` | built in | Validates syntax, imports, references, types, inheritance, composition, exports and cycles. Runs build hooks for their diagnostics. Writes nothing. Exits with 1 on errors. A path is a file or a glob. The default is the project. |
| `build [config] [--dry-run]` | built in | Writes renderer output and plugin build output. |
| `compile <path> [--renderer json\|yaml\|markdown] [--write] [--dependencies]` | built in | Compiles one file to stdout, with a key for each export. |
| `format [path] [--check]` | built in | Writes the canonical format. |
| `reach [targets...] [--no-unreachable] [--no-paths]` | built in | Shows what is reachable and what is not. |
| `slice <target>` | built in | Shows the spec that one item needs, as one Markdown document. |
| `init [dir] [--template group/name] [--adapter id...] [--list] [--no-tether]` | built in | Makes a new project from a template. |
| `loc [paths...]` | built in | Shows total, code, comment and blank lines for each file, and a summary. Exits with 0. |
| `lsp` | built in | Starts the language server on stdio (16). |
| `plugins` | built in | Section 11.9. |
| `tether`, `update`, `untether`, `remove` | built in | Section 13. |
| `agent [claude\|codex\|opencode\|muse] [--model/-m [M]] [--print-fluency] [args...]` | Belay | Starts an agentic tool with this prompt. |
| `belay slice <target> --adapter <id>` | Belay | A slice that cites the compiled output of one adapter. |
| `test` | test plugin | Section 14. |
| `migrate [--check]` | migrate plugin | Section 11.12. |

**`build`**:

- If no `entry` is set, the entry is `index.pi` in the root. If that file is
  not there, the result is an error.
- Without a building plugin, it writes `<output>/<path below root>.<ext>` for
  the entry and for each file that holds an item its exports refer to.
  Example: `spec/components/Button.pi` → `dist/components/Button.json`.
- Plugin output replaces the renderer output. If you set `output` or
  `renderer`, Piton writes both. A Claude Code project gets `.claude` and
  nothing in `dist`.
- The build validates the full plan before it writes (11.3).
- `.piton/manifest.json` is in `.piton/`, next to `piton.config.pi`. Its shape
  is `{generated: [...], targets: {targetId: documentationChecked}}`.
  `targets` records the date when someone last checked the documentation of
  each target.

**`compile`**:

- `--renderer` has the default `json`.
- It does not show items without an export, or abstract anchors.
- `--write` writes the output next to each input. Glob paths need `--write`.
- `--dependencies` wraps the output as
  `{"value": "<output string>", "dependencies": [abs paths]}`. It does not
  include bundled files.

**`format`**:

- The canonical format: 4 spaces, constraint spacing, sorted and wrapped
  imports, no `.pi` in paths, a space after `//`.
- Without a path, it formats all sources below the source root. It does not
  change the output, dependency directories or installed packages.
- `format -` reads stdin and writes stdout.
- **It only splits long lines. It never joins paragraphs again.** When you
  edit prose, wrap it again by hand. Do not wrap `key: value` blocks as
  prose.

**`reach`**:

- It starts from files, globs or anchor names. The default is the project
  (its entry).
- It follows imports, references, inheritance and composition. This includes
  variables that a value embeds and properties that a value reads.
- It shows each reachable item with its depth and path. It also shows
  unreachable items. Results are in groups by source, with direct and
  indirect reach.
- It counts each anchor one time. An anchor that lost a duplicate export
  counts where its declaration is.
- Without targets, the roots are the entry exports and the anchors that
  plugins reach (11.11), for example tests. With targets, plugins add
  nothing.
- `piton.config.pi` is read, not imported. It is never unreachable.
- Piton parses files that nothing imports (it does not compile them) to
  count their anchors. It lists them as `(never imported, N anchors)`.
- Summary: `37 anchors reachable, 412 unreachable (380 in 50 never-imported
  files)`. The unreachable count includes those files. The parenthesis shows
  when such files exist, also when their anchor count is 0.
- Then: `53 modules loaded, 2 carrying nothing reachable, 50 never imported`.
- Use it to find dead spec. It gives no diagnostics and always exits with 0.

**`slice`**:

- Target forms: `file.pi#Anchor`, `file.pi#Anchor.property`,
  `file.pi#variable`, or a bare name that is unique in the project.
- Piton finds the project through the nearest `piton.config.pi` above the
  current directory.
- It follows these dependencies, transitively:
  - A full anchor needs all its properties and each anchor that it extends.
  - A property needs each base that declares that property.
  - Constraint types, values that it reads, references and embedded items.
  - A reference to an anchor needs the full anchor. A reference to a
    property needs only that property.
  - A variable needs what it reads, refers to or embeds, and its constraint
    anchors.
  - Imports alone add nothing.
- A property slice does not show the other properties of the anchor.
- It adds the **chain** from the entry to the target. The chain is the
  shortest build path, with one property for each link (for example
  `Specification.Tooling`, `Tooling.cli`, `Cli.commands`). Piton traces it
  from the entry also when the target is a file. Entry exports, variables
  and files that the build does not reach have no chain. The introduction
  names the links of the chain.
- Order: the target, then dependencies with the nearest first (source order
  for ties), then the chain with the outermost first.
- Each declaration shows one time, with its name, file, bases, type and
  reads. Values look as the Markdown renderer writes them. References and
  embedded anchors show as names.
- Piton follows a cycle until it comes back to the start, and lists each
  dependency in it. Markdown links that you wrote stay the same.
- Paths are relative to the current directory. The output is deterministic.
- Errors: a file that is not there, a name that is not declared or imported,
  a property that is not there, a bare name with more than one match, a path
  that goes into a property. It shows errors only, not warnings.
- It exits with 1 on errors.

**`belay slice`** (Belay plugin):

- `piton belay slice <target> --adapter <id>` (for example `claude-code`)
  cites the compiled output of one adapter, not the `.pi` files.
- A bare name is looked up in what the build compiles.
- Piton compiles the project from its entry as a build does. You do not have
  to run `piton build` first. The target must be an item that the build
  writes.
- Each declaration is shown in its file below its heading. Citations become
  links, for example `.claude/reference/ui.md#color`. Paths are relative to
  the current directory, for example
  `../../.claude/reference/components/Button.md#button` from
  `spec/components`.
- An embedded anchor is shown in the section that embeds it.
- Piton never cites the source. Abstract items and items that are only read
  have no location. Names that cite them are not links.
- Belay gives the layout. The host finds and prints the slice.
- Errors: no Belay config, a target that the project does not build, a
  project with errors, a file that the build does not reach, a name that the
  build does not compile, an anchor that Piton never writes, and all
  `piton slice` errors. It exits with 1 on errors.

**`init`**:

- Templates are in the binary. `init` needs no network for them.

| Template | Content |
| --- | --- |
| `crux/project` | The preferred group. It is first in the list, marked "recommended". Belay, with https://github.com/piton-lang/crux.git declared. `init` tethers it, and the entry exports it again. |
| `piton/minimal` | JSON in `dist/`. |
| `piton/library` | A package that other projects install with `piton tether`. |
| `belay/application` | A shape instruction in `src/`, a skill, an agent and a command. |
| `belay/minimal` | The Belay config and one skill. |

- Piton makes the directory if it is not there.
- Without a directory, it asks for the project name. An empty name or no
  terminal means the current directory.
- It asks for the name before the template.
- In an interactive run, it asks for the group, the template (lists that you
  can filter) and the adapters. The defaults of the template are checked. You
  must select one adapter or more. The checklist shows the platform and the
  target id.
- A group with one template does not ask. `--template belay` (only a group)
  asks for the template.
- A non-interactive run needs a full template name. Without `--adapter`, it
  keeps the defaults.
- `--list` lists groups and templates with descriptions, and does nothing
  more.
- `--adapter` can repeat or take commas. It works only for Belay templates.
  A Belay template is a template whose config lists adapters.
- The selected adapters change the import and the `adapters` list of the
  config (in the export order of `@piton/belay`, formatted).
- Piton writes `.adapters/<id>/` files only for selected adapters, for
  example the `opencode.json` of OpenCode.
- After it writes files, `init` tethers the declared `dependencies`. This
  needs the network. If tethering fails, the files stay, Piton tells you to
  run `piton tether`, and it exits with 1.
- `--no-tether` skips tethering. The next steps then include `piton tether`.
- Templates are in `create-templates/<group>/<name>`. A template directory
  holds exactly the files that a new project gets. `.template` files hold
  descriptions and Piton does not copy them. `.preferred` marks a group. To
  add a template, add a directory.
- Each template must check, format and build without errors for each
  adapter.
- `init` never overwrites a file. It names the conflicts and writes nothing.
- Esc or Ctrl+C cancels before Piton writes.
- Paths and tether output go to stdout. stderr shows the template, adapters,
  file count and next commands.
- Errors: an unknown template; a group or no template without a terminal; an
  unknown adapter; `--adapter` on a template that is not a Belay template;
  cancel; overwrite; tether failure.

**`agent`** (Belay plugin):

- The default tool is `claude`. `args` is a trailing argument. All that
  follows goes to the tool.
- It starts the tool in the project root with the terminal, and waits for
  it. The prompt is a project primer and this fluency reference.
- The reference is the `FLUENCY_PROMPT.md` of the project. Without it, Piton
  uses `plugins/belay/fluency.md`, the copy in the binary.
- `--print-fluency` prints the full prompt and starts nothing.
- The primer gives the source root, the reference root and the instruction
  file:
  - If the project builds for the adapter of the tool, all three belong to
    that adapter.
  - In a shared project, the reference root belongs to the first adapter,
    unless Piton wrote the reference tree of the tool.
  - If the project does not build for the tool, all three belong to the
    first configured adapter.
- Piton adds the text to the instructions of the tool. It never replaces
  them:

| Tool | Method |
| --- | --- |
| `claude` | `--append-system-prompt` |
| `codex` | `-c developer_instructions=<TOML string>` |
| `opencode` | `--standalone`, with `OPENCODE_CONFIG_CONTENT` that lists a temporary file (private server). Piton removes the file when OpenCode exits. |
| `muse` | `-c run.developer_prompt=<JSON string>` |

- For `muse exec` and `opencode run`, the options go after the subcommand.
- Subcommands that refuse the options (`muse resume`, `opencode models`) run
  without the prompt.
- If the tool is not on PATH, the result is an error. The error tells you to
  use `--print-fluency`.
- The exit status is the exit status of the tool.

Models (`--model`, `-m`):

- Piton gives the model to the tool with the option of the tool. The model
  option goes with the prompt options, also after a subcommand.

| Tool | Model option | Model list |
| --- | --- | --- |
| `claude` | `--model <model>` | The aliases `fable`, `opus`, `sonnet`, `haiku`. Claude has no list command. |
| `codex` | `-c model=<TOML string>` | The slugs of `codex debug models`. |
| `opencode` | `--model <provider/model>` | The lines of `opencode models`. |
| `muse` | `--model <model>` | The catalog id in its help: `muse-spark-1.3`. Muse has no list command. |

- A tool that is not on PATH, or whose list fails, lists no models. The note
  says so.
- Without `--model`, on a terminal, Piton shows a menu of the models.
  "Default model" is first and keeps the default of the tool. If you cancel
  the menu, nothing starts.
- Without `--model` and without a terminal, the tool uses its default model.
  A note lists the models and tells you to select one with `--model`.
- `--print-fluency` starts nothing, so it lists no models.
- `--model` with no value lists the models, one on each line, on stdout. It
  starts nothing and exits with 0. If the tool lists no models, the result
  is an error, and the exit code is 1.
- A `--model` value that is a model of the tool is that model.
- Another value is a search string. Piton lists the models that contain it
  (case is not important), one on each line, on stdout. If no model contains
  it, Piton lists all models, and the outcome line says that nothing
  matched. It starts nothing and exits with 0.
- If the tool lists no models, Piton cannot search. It gives the value to
  the tool as the model.

## 16. Language server, editors, JavaScript

### 16.1 Features

`piton lsp` gives these features:

- Diagnostics for Piton. It does not report optional properties that are not
  there.
- Plugin diagnostics (Belay): the server plans plugin builds when you save a
  file, never while you type. It shows the diagnostics of the last save. A
  plugin that cannot plan is reported on the config.
- Completion, hover, go to definition, type definition, document highlight,
  find references.
- Workspace rename, module rename, auto import, and import organization
  (unused, duplicate, not valid, too broad).
- Code actions: import a symbol, create an anchor, add an export, qualify a
  reference, resolve a simple inheritance conflict. Whole-file actions:
  organize imports, format.
- Document symbols and workspace symbols.
- Semantic highlights, inlay hints, format (full file only, minimal edits),
  folds, selection ranges.
- Hierarchy and provenance views, override lenses.
- A preview of compiled output: Markdown, JSON, or the plugin build output
  (for example the files of each Belay target), from the last save.
- Source-to-output map. It shows in hover ("Compiled to") and in a code lens,
  never as document links.
- Detection of unused and redundant items. "Used" means what `piton reach`
  reaches: the entry exports and the anchors that plugins reach (11.11).
  Thus an exported test is not unused.
- Detection of conflicts and of cycles. Cycle detection finds cycles that
  cannot resolve and anchors that extend themselves. It shows the cycle that
  caused the error. It does not report circular imports.
- Expression checks: a warning when an expression cannot give the required
  type.
- Export checks, links to related symbols (from an abstract to its
  implementations, from a base to its extensions), and inspection of
  resolved values.
- Documentation in hover, completion, symbols and hierarchy.
- An index of each source below the project root and of open files. If the
  entry is not there, the server reports it and loads all other files.
  Without an entry, it reports nothing as unused.
- No signature help.
- The server is not incremental. It compiles again after a pause of 250 ms in
  typing. A compile takes tens of milliseconds.

### 16.2 Protocol

- Positions use UTF-16. The server offers no other encoding.
- The server syncs the full buffer. Diagnostics and edits have versions. The
  server refuses old edits. It does not apply them incorrectly.
- Semantic tokens, inlay hints and lenses refresh after a save, a change on
  disk, a config change or a closed buffer. Typing does not cause a refresh,
  because the editor asks again for that buffer.
- Format, folds and selection read the buffer as it is. They never wait for
  a compile.

### 16.3 Feature rules

**Completion**:

- It uses the line rules of the parser. `-word` (a list item without its
  space) is a sentence.
- It never offers items that do not exist.
- It offers nothing in these places: an empty value (`property: `), after
  `.` in a string, comments, prose, escapes, quoted strings, a declared name,
  a keyword after `as`, a nested dictionary.
- **It offers each item one time.** It offers a name that a module exports
  again one time, imported from the shortest path. It offers each different
  anchor with the same name, and names its module.
- In a body, it offers the properties that are not there (required first).
  It does not offer properties that are there. It offers `pass` only in an
  empty body.
- A key that you complete before a colon that exists keeps one colon.
- It sends the insert range and the replace range. If you accept in the
  middle of a word, it replaces the full word.
- It adds documentation later, on request. Items older than the last compile
  get no documentation.

**Hover**:

- It shows only on symbols, declared names and `+` or `++`. It never shows
  on prose, literals, language keywords, comments, escaped text or names
  that do not resolve.
- It shows the type, the source (a link relative to the workspace),
  documentation, the inheritance chain, the export status and the compiled
  value.
- It tells if a property is inherited, is an override, or implements a
  required property. To implement a required property is not to override it.
- A required property says that you must write it.
- `this`, `self` and `super` tell what they mean.
- "Compiled to" shows only on the declared name.

**Go to definition**:

- A module path opens its file. A keyword goes to its anchor.
- `this` and `super` members go to the anchor that gives them. An override
  goes to the item that it replaces.
- The answer also tells which word the server followed, and gives the full
  target declaration.

**Type definition**: it goes to the constraint anchors in the written order.
An anchor goes to itself. Built-in types go nowhere.

**Document highlight**: it marks the declaration differently from reads.

**Rename**:

- Anchor names: `[A-Za-z_][A-Za-z0-9_]*`. Variable names and keys can also
  have `-`. Keywords are lowercase or kebab-case.
- Reserved words are not permitted.
- The server refuses collisions and bundled symbols, and gives a reason. It
  refuses a property name that the family already uses.
- An alias and its original are different names.
- To rename a module path, move its file. The server never changes the
  module path on an import line.

**Module rename** (move a file or folder):

- The server changes each `use` and `from` path that names it, before the
  move. It also changes the relative paths in the moved file.
- Each path keeps its style. Rooted paths stay rooted. Relative paths stay
  relative. `.pi` stays written or omitted. A path to the index of a folder
  stays a path to the folder.
- Package paths never change.

**Symbols**:

- Document symbols show constraints as written. Dictionary keys are nested
  below. List items are not symbols.
- Workspace symbols: case is not important. The order is exact match, prefix,
  substring, then subsequence. Shorter names come first. The maximum is 256.
  Bundled names are not included. Each symbol names its module relative to
  the workspace.

**Semantic tokens**:

- They cover only resolved names: anchor = class, property or key =
  property, variable, user keyword = keyword. A module path is a namespace.
- Names that do not resolve get no token. Prose gets no tokens. In a value,
  only expressions in `{...}` forms get tokens.
- The syntax highlighter shows all lexical items, with scopes that agree.

**Inlay hints**:

- The type hint shows the inferred type of an unconstrained value that is
  one `{}` expression, as `:: type`. It applies to properties and variables.
- The owner hint `← Owner` shows after a member that another anchor gives.
  It applies to members read through `this`, `super` or an anchor, only if
  the owner is not that anchor. `self` shows no hint.
- The server never hints literal types, overrides or composition operators.

**Code actions**:

- Each action relates to an item at the cursor: a name that does not
  resolve, a header line, or an import. Plain text gets no actions.
- Whole-file actions show when the editor asks for that kind. Organize
  imports also shows when the cursor is on an import.

**Format**: the server does not format part of a file, because that can
change the structure.

**Overrides**: an implementation of a required property gets no lens. The
lens opens the declaration that the override replaces. "Find
implementations" lists the anchors that write the property themselves.

**Selection ranges**: they grow from a name through each expression around
it, the braces, the value, the property, each block around it, the
declaration and the file. Each step is wider than the last. No step includes
blank lines around it.

**Folds**: declarations, block properties, escape blocks (with their fence),
and groups of 2 or more comment lines or import lines. A fold covers full
lines from the first to the last content line. A header that is not complete
still folds.

### 16.4 Editors

Editors must:

- Indent one level after Enter on a colon that opens a block.
- Remove one level of indent after Enter on a blank line in a block.
- Optionally format on save. This adds the space after `//` and changes
  nothing more in comments.

Highlighters must:

- Show prose in the plain text style. Imports and declarations are only at
  column 0. An indented line such as `use the spec to decide` is prose.
- Show `$`, `#`, `@`, backticks and brackets without a pair in prose as part
  of the string.
- Show a value that is exactly a number, boolean or null as that type. With
  other text next to it, it is prose.
- Show a quoted string in `{...}` as a string.
- Show code fences as usual Piton. Show escape blocks as verbatim text. They
  can optionally use the highlight of the fence language.
- Agree with the classification of the parser. Tests check this
  (`editors/fixtures/`). A syntax definition can differ from the parser only
  where a known, listed gap tells why.

| Editor | Highlighter | LSP |
| --- | --- | --- |
| VS Code | TextMate | Full |
| Emacs, Helix, Zed | tree-sitter | Full |
| Neovim, Vim | vim syntax | Full |
| JetBrains | `jetbrains` | Full |
| Sublime | `sublime-syntax` | Full |
| Kate | `KSyntaxHighlighting` | Full |

The editor files are below `editors/`.

### 16.5 JavaScript

- `packages/vite-plugin-piton` imports `.pi` files as modules. It supports
  HMR, dependency tracking (through `compile --dependencies`) and virtual
  modules.
- Each export is a named export. The default export is the full file, the
  same as the output of `piton compile`.
- You can configure the default renderer (`'json' | 'yaml' | 'markdown'`).
  You can also import the renderers as functions from
  `vite-plugin-piton/renderers`.
- `packages/astro-piton` wraps the plugin for Astro.

```ts
import { SaveButton, pi } from '../spec/app.pi';
import spec from '../spec/app.pi';
```

`spec.CancelButton` reads one export from the default export.

## 17. The agent tools of this repository (`spec/agent/`)

`spec/agent/index.pi` exports these items. Belay compiles them for the agent.

| Item | Purpose |
| --- | --- |
| `skill GenerateFluencyPrompt` | Makes this prompt from the full spec and writes it to `${FLUENCY_PROMPT_FILE}` (`${BELAY_PROJECT_ROOT}/FLUENCY_PROMPT.md`). The prompt must be small but complete, in ASD-STE100 Simplified Technical English. |
| `skill BuildTooling` | Reads the spec in `.claude/reference` and builds the tools. The stack is Rust, chumsky, rowan and tower-lsp. |
| `skill InspectSpec` | Reads `spec/` and answers questions. Without more instructions, it summarizes the language and reports spec problems. It saves reports as standalone HTML in the current directory. Each problem names the files that cause it. |
| `command CheckSpecForProblems` | Reads `${BELAY_PROJECT_ROOT}/spec`. It finds parts that are underdefined, overdefined, contradictory or badly organized. It shows the problems one at a time. The user selects the source of truth or adds a clarification. |

`xtask` commands (from BuildTooling):

- `xtask install` builds the release and installs it on the local machine.
- `xtask sync-fluency` makes `FLUENCY_PROMPT.md` again. Then it copies it over
  `plugins/belay/fluency.md`. That is the copy in the binary that
  `piton agent` uses if the project has no prompt. Then it tells you to build
  again.
  - It runs `claude -p` headless with the GenerateFluencyPrompt skill in the
    repository root. Claude can read, search and write, and Piton accepts its
    edits. Claude cannot run commands. `--model` selects the model.
    Without it, Claude uses its default model, and the task lists the Claude
    models (the aliases of `piton agent`) and tells you to use `--model`.
  - Piton shows the output of Claude when it ends. The person who runs the
    task reads the diff to see if the prompt is correct.
  - If Claude fails, is not on PATH, or ends without a non-empty file, the
    result is an error. Piton copies nothing.
  - If the prompt did not change, Piton tells you, and it still copies it.
  - `--copy-only` does not make the prompt. It only copies.
  - `--check` makes the prompt and copies nothing. It fails when the two
    copies are different and names the command that fixes it. Use it in CI.

The config of this repository lists `BelayConfiguration` under `plugins:`
(`codeRoot: ./crates`, `shapeRoot: ./spec/shape`, Claude Code). It offers the
package `piton` (`root: ./spec`).

The spec also holds tests (`spec/tests/`). Examples:

- `CommandsArePrefixed`: commands start with `x-`. On Claude Code they are
  skills with `disable-model-invocation: true`.
- `SkillDescriptionJoinsUseWhen`: the skill description is the description,
  then "Use when", then `useWhen`.
- `CheckWritesNothing`: `piton check` never writes a file.
- `TestsWithoutAVerdictAreErrors`: a session without a verdict is an error,
  not a failure.

## 18. Work on a specbase

1. **Learn the project.**
   - Read `piton.config.pi`: root, entry, output, plugins (Belay settings:
     adapters, codeRoot, shapeRoot; test settings), packages, dependencies.
   - Read the entry, usually `spec/index.pi`. In this repository it exports
     `Specification` (FoundingThesis, Language, Tooling, Belay,
     PackageManagement, Plugins, Migration, Testing) and the items of
     `spec/agent/`.
   - Read down through the `index.pi` files.
   - The Belay constructs and adapters are in `plugins/belay/pi/`. The test
     anchors are in `plugins/test/pi/`.
   - Compiled reference files give an overview. The source is the truth.
   - `piton slice` gives a focused view of one item.
2. **Edit the `.pi` source.** Obey the local conventions:
   - One exported anchor in each file, with the file name of the anchor.
   - Index files use `from ./X export *`.
   - Export abstracts with a keyword. Put shared abstracts in `lib/`.
   - Property names are lowerCamelCase. Exported constant lists are
     SCREAMING_CASE.
   - Put prose in string blocks. Put structure in keys and lists.
   - Use `@{X}` for links.
   - Wrap fenced examples in `\\\`.
3. **Write the imports.**
   - Use `use` for keywords and `import` for names. Frequently you need both.
   - A new file compiles only if the entry reaches it. Tests are different:
     Piton finds them below the full source root.
4. **Validate.**
   - Run `piton check`.
   - Compile each spec example that you edited with `piton compile`. Compare
     it with its ` ```json ` block. Piton does not check escape blocks.
     Remember that only exports show.
   - Run `piton build` (or `piton build --dry-run`) and read the diff of the
     generated files.
   - Run `piton reach` and `piton format --check`.
   - Run `piton test` if the project has tests.
5. **Do not edit generated files.** These are:
   - `CLAUDE.md` and `AGENTS.md` below `codeRoot`.
   - `.claude/`, `.codex/`, `.agents/`, `.opencode/`, `.muse/`.
   - Build output that `.piton/manifest.json` lists, and `.piton/` itself.
6. **The spec and the implementation must agree.** If they do not agree,
   report it. Do not change the spec to fit the code. Use `piton compile` to
   see the current behaviour.

## 19. Compiler and spec

The spec does not name these items. The compiler has them:

- Most diagnostic codes in this document. The spec says only "error" or
  "warning". The codes are the names that the compiler and the plugins use.
  `duplicate-export`, `unsupported-construct`, `frameworks-removed`,
  `invalid-test-scope`, `unknown-test-agent`, `invalid-test-model` and
  `invalid-jobs` are in the spec.
- The CLI flags `reach --no-unreachable`, `reach --no-paths`, and
  `format -`.
- `check` with a directory as a path. Without a config, `check` checks all
  `.pi` files below the current directory.
- The list of constructs that need a `description` for each adapter. It is in
  `plugins/belay/src/adapters.ts` (`descriptionRequired`), not in the
  adapter `.pi` files.
- The `remove` rules (refuse if edited or imported) and the `dangling-pin`
  and `unknown-pin` codes.

Differences and limits:

- A paragraph break compiles to one `\n`. Markdown shows this as a soft break
  in one paragraph. The spec shows two paragraphs.
- The compiler splits digits in headings: `t1` → `T 1`.
- An import with a `/` root in an installed package resolves against the
  root of the consumer. Use relative imports in packages.
- A value from a concrete base does not satisfy a required abstract
  property. The abstract of the keyword comes last, and its declaration
  without a value wins. Set the property on the anchor itself.
- `piton check` without paths checks only what the entry reaches. Name the
  files that it does not reach.
- **Prose directly in an anchor body (with no key) disappears without an
  error.** Put it below a key such as `description:`.
- `==` on two different types gives false (`{1 == "1"}`). It is not an error.

Problems in the spec. Report them if your work touches them:

- `spec/scope/tooling/index.pi` holds a placeholder property
  `something: HAHA TOOLING`.
