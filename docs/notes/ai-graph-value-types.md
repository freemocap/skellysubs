# Graph value types: specification review

Date: 2026-10-09. Status: reviewed implementation contract; implementation and
verification tracked separately in the production integration note.

## Reason and scope

The initial core profile supports Boolean, Integer, Text, List and exact Record.
It cannot directly represent existing native assessment probabilities/confidence
(`ChoiceAssessment`), nullable feedback scores (`ConversationFeedback`), or their
string-keyed evidence maps. Encoding these as strings or custom lists would add
conversion representations between native domain code and its graph artifact.
This is a concrete blocker to migrating the remaining operations, not a change
to assessment policy or a general platform expansion.

Specification review against the foundations: values remain typed by exact named,
versioned contracts. Extend the shared value algebra with three constructors:

| Constructor | Accepted values | Explicit exclusions |
| --- | --- | --- |
| Number | JSON numbers representable as finite `f64`, including integers | Numeric strings, null, booleans, non-finite numbers |
| Nullable(T) | JSON null or a value accepted by T | A missing port or missing required record field |
| Map(T) | JSON object with string keys and every value accepted by T | Arrays, null, mixed incompatible values |

Values are validated without conversion, rounding, filling fields or rewriting
numbers. Number is a shape, not a compatibility exception: Integer and Number
contracts cannot connect merely because their value sets overlap. Existing
Integer and exact Record semantics remain unchanged. Maps do not make records
open; undeclared record fields still fail. Domain validators continue to enforce
probability ranges/sums, allowed evidence keys and other semantic refinements.

Nullable values are present values. They do not mean skipped, pending, unavailable
or absent. Optional ports continue to express absence independently. No scheduler,
sharing, guard, adoption, source-authority or execution-identity rule changes.
Reuse continues to use existing encoded values; no new numeric normalization or
equivalence between differently encoded numeric inputs is introduced.

## Persistence and visualization

Old constructor encodings and old artifact fingerprints remain unchanged.
Checkpoints whose artifacts use new constructors require format 9, even before
the first invocation. Formats 1–8 reject artifacts containing these constructors;
old formats remain readable with their original checksums and semantics. New
format survives compaction, recovery and handler-free history inspection.
No existing user record is rewritten or deleted. If integrating these artifacts
requires an incompatible workspace storage contract, add the consecutive workspace
migration before application admission; do not reset or reinterpret old data.

Generate frontend types from the native exporter. The viewer receives the same
artifact constructors as execution; it must not infer alternative type semantics.
No permissive Any/opaque JSON constructor, JSON-schema interpreter, implicit
coercion or new language-specific rule is introduced.

## Required invariant checks

- Nested accepted/rejected values for all three constructors, including null
  versus absence, exact records versus maps, and integer versus number contracts.
- Native Begin, invocation, settlement and adoption preserve values exactly;
  invalid outputs fail and absent required inputs remain errors.
- Existing artifact serialization/identity is unchanged. New constructors affect
  artifact identity and appear in the native inspection artifact.
- Format-9 capture/replay and compacted history retain typed values. Older format
  tags reject the new constructors rather than accepting an unsupported contract.
- Generated contracts, native core tests and affected domain regressions pass.

These checks establish the implementation guarantees; this review alone does not
claim those checks have passed or that application workflows have migrated.
