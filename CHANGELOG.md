# Changelog

## [0.1.1] - 2026-08-19

### Added

- Input that reads as a Rust source file is rejected as unsupported input, exit
  40, at two layers: a `.rs` path under any profile, and content whose lines
  outside code blocks carry Rust structure at eight lines and 35 percent of
  non-blank lines. A line counts on an attribute or comment opener, on
  structural punctuation alone, or on a code terminator paired with an item
  keyword, a path, arrow, or fat-arrow token, or a field line. A field line is
  one identifier, a colon, one type expression, and a comma, so a definition
  list writing several words after its colon stays prose. The prose and code
  split is the extractor's
  own segmentation, so backtick-fenced, tilde-fenced, and four-space indented
  samples all stay prose. The scope is Rust shape only: source in another
  language reaches the rules and produces findings a reader should discount.
  The guard catches a mistake and is not a security boundary. Counting comment
  lines closes most of the prefix evasion, and a writer determined to get past
  it still can. The error carries both remedies, which are to pass the prose or
  to wrap the code in a fenced block.

### Changed

- SLOP-A002 reads every homograph's past tense the same way it reads that
  verb's present tense. `harnessed` rides the structural verb-context arms,
  while `navigated` and `landscaped` join the word set beside `navigate` and
  `landscapes`. A past participle standing as an adjective is not the verb and
  never fires, marked by a hyphen-joined left edge, a determiner or possessive
  directly before it, or an `-ly` adverb: `well-navigated waters`,
  `a landscaped garden`, and `the carefully navigated channel` are all silent.
  The `navigate` collocation exemptions carry every inflection, so
  `navigated the file` is exempt exactly as `navigate the file` is.
- The skill states the two-part keep test for contrast. Keep a contrast only
  when both halves change what the reader thinks or does, and only when the
  sentence stands after the rejected half is cut. Pattern 9 names SLOP-C003
  beside the other contrast rules.

### Fixed

- SLOP-C004's sentence-boundary arm requires a real boundary in both
  directions. A digit run that opens its line is a list marker and never a
  sentence end, while a sentence that ends on a numeral is one, so
  `version 2. Granted, ...` fires where it used to be silent. An abbreviation
  still never opens a match, on the terminal-period test SLOP-C007 uses. The
  boundary also has to sit inside one block, so no finding reaches from the end
  of one block into the next and drags a list marker into its span.
- A rule's `exemptions` table fails the policy load when it cannot match. A
  value that is not a table, a table with no keys, a key whose value is not an
  array, an empty array, and an empty phrase all stop the load. A dead
  exemption is worse than a missing one, because the guard text goes on
  promising it.

### Documentation

- Policy 0.1.1. The snapshot reference is regenerated.

## [0.1.0] - 2026-08-19

First release. A deterministic linter for the patterns that mark writing as
machine-generated, with 91 rules in 22 families, six writing profiles for
essays, blog posts, email, reports, documentation, and short posts, JSON and
text output, and an agent skill that runs the adjudication loop.
