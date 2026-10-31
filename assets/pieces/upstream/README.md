# Verbatim upstream artwork — do not edit

The twelve files here are Cburnett's chess pieces exactly as published on
Wikimedia Commons. **Nothing in this directory is ever edited**, and nothing
reads it at runtime.

It exists so that the modification the licence asks us to state can be checked
rather than believed: `assets/pieces/basic/` is generated from these by
`tools/pieces/normalise-upstream-pieces.mjs`, and

    node tools/pieces/normalise-upstream-pieces.mjs --verify

asserts that every path in every generated file is identical to the one here.

Licence, provenance and what we changed: [`../LICENCE.md`](../LICENCE.md).
