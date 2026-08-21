# Changelog

## [0.1.3] - 2026-08-20

### Added

- A seventh profile, `comment`, for a reply in a thread: a pull request comment,
  an issue comment, a review note. Eleven rules move and nothing else does. The
  assistant register runs stricter here than in email, because a thread already
  names who is speaking and already carries the addressee, so a sycophantic
  opener, a turn-taking offer, a valediction, and a correspondence offer all
  fire at full strength. Voice runs softer in return: a metaphor noun, an emoji,
  a courtesy closing, capitals, a verification claim, and a line saying what the
  writer ran all reach a judge question and none of them blocks. Softeners are
  off, the structure and link-density rules are off, and length carries a
  400-word cap that reports without gating. Rule 3 applies, because a sign-off under a
  reply is letter furniture that arrived with a paste.
- `SLOP-V002` reads eight praise entries at the opening of a sentence, a line,
  or a list item and nowhere else: `great question`, `good question`,
  `excellent question`, `that's a great question`, `great point`,
  `excellent point`, `you're absolutely right`, and `you are absolutely right`.
  Four of them are new here and the other four moved under the anchor with
  them, so the set holds together and no single entry is left as a way around
  it. Away from the opening these are ordinary English, so a line reporting
  that someone asked a great question stays silent.
- `SLOP-V002` gains `fair hit`, the one concession entry, which carries no
  anchor because opening a reply with it and writing it mid-sentence are the
  same tell. Two collisions reach the judge and no exemption stands behind
  them. One is the literal sense in sport. The other is the word `unfair`,
  which contains the entry and matches under substring reading.
- A gate on the rule guards themselves. Every `guard` and `judge` string in the
  policy package is checked for em dashes, en dashes, semicolons, contrast
  scaffolding, and banned filler, and the test suite fails on any of them,
  naming the rule and the offending substring. The check reads those two fields
  and nothing else, so a regex literal or a lexicon entry never raises a finding
  and needs no exemption. A guard naming a term the package declares is a
  mention and passes, which is how the filler rule can quote its own entries.
  Punctuation has no exemption path, because no rule declares a semicolon.
- `SLOP-C011` reads a fourth spelling of a denied capability. A segment that
  opens on `and` and then denies a capability in the base form has the shape of
  a command, and the subject standing in an earlier segment of the same sentence
  is what tells the two apart. A line saying the rules read text and never
  detect authorship now reports. A line saying read the report and never judge
  by one finding still does not. Only `and` opens the shape: a comma, a `but`,
  or a `so` in that seat licenses a real imperative and stays silent.

### Changed

- The block-start test reads past a leading run of emoji, symbols, and
  whitespace. The semantics block always said the position is read after
  markers are stripped, and a decoration at the head of a line is a marker,
  so a phrase behind one now sits where the reader sees it. This reaches every
  rule anchored to a block start, and six of them report where they used to
  stay silent. `SLOP-V002` reads a praise opener behind an emoji, and
  `SLOP-T001` reads `overall` the same way. `SLOP-M003` reads an opening
  `however`, `SLOP-T002` reads a transition opener such as `moreover`,
  `SLOP-S001` reads an attribution line, and `SLOP-S004` reads a valediction.
  Four bullet glyphs join that run: `•`, `‣`, `⁃`, and `∙`. The shapes a
  nested level uses, the white bullet and the small square and the black
  circle, were already read past because they sit in a range the list carried
  whole, while the plain bullet sat in a different Unicode block and was not.
  Nothing intended that split, and a pasted list should not depend on it. Only
  `•` has a measured population, and it is small: 5 occurrences with none
  line-leading in a 738k-line repository corpus, 39 with 2 line-leading in a
  504k-line prose corpus. The other three are here for completeness by
  analogy, with no line-leading occurrence in either corpus. The middle dot
  `·` stays out. It is a letter in Catalan and a separator inside running
  prose, it appeared 232 times across the same corpora with none of them
  opening a line, and a rendered list does not paste as one.
  `SLOP-D004` follows from `SLOP-T002`, whose hits it counts, so three
  decorated transition openers in one piece now reach its threshold where none
  of them used to count at all. Every one of the seven moves in the same
  direction, from silent to reported, and none moves the other way. Two corpus
  passes measured no change at all between the old behavior and the new,
  because neither corpus carried a decorated opener, so the seven are measured
  on constructed inputs instead and the corpora say only that the shape is rare
  in repository text.
- Text input has its list, quote, and heading markers read as structure, the
  way markdown input always did. A marker-led line used to
  carry its marker into the text a rule reads, so every rule anchored to the
  opening of a line saw the position after the marker instead of the position a
  reader sees. Both input formats now report the same finding on the same
  bytes. A marker with no space after it is ordinary text, so a horizontal
  rule, a negative number, and a hashtag are untouched.
- The crate ships `examples/score.rs`, the instrument that produced the two
  raw-source guard thresholds. `cargo package` reports no ignored files, and
  the example builds from the unpacked archive, so anyone can rerun the
  measurement behind those constants.
- The rename hint reads the old profile name without regard to case, so
  `--profile Essay` and `--profile ESSAY` say where the name went. Valid input
  is still lowercase only, there is still no alias, and the exit code is still
  2.
- `SLOP-V002` drops `good catch` and `great catch`. A reviewer who writes
  either one means it, and the honest use opens a line, so no anchor could
  have told the two apart. The paired residue still reports, because a line
  reading `Good catch! You're absolutely right.` fires on the second phrase.
- The `essay` profile is renamed `general-writing`. There is no alias. Running
  `--profile essay` is a usage error with one extra line saying where the name
  went. The profile keeps its stances, its index, and its place as the catch-all
  for anything the table does not name.
- `SLOP-C004` splits its line-start arm by word. `although` and `though` have no
  temporal reading and fire as they did. A `while` clause now drops on either of
  the two shapes that mark time passing. One is a progressive before the comma,
  so a line saying while you are working, you might notice unexpected changes is
  silent. The other is a participle straight after the keyword, so lines saying
  while working on the migration and while redistributing the build are silent
  too. Eight concession participles are exempt and keep firing, because a
  concessive clause reaches for a verb of cognition where a temporal one
  reaches for an activity: `acknowledging`, `recognizing`, `granting`,
  `accepting`, `conceding`,
  `admitting`, `noting`, and `allowing`. A line saying while the parser is
  slower, it handles more cases still reports, and the bare durative present, as
  in while the build runs, grab a coffee, still fires and is answered at the
  judge question. One adverb may stand between the form of `be` and the `-ing`
  word, which is where a writer puts `still` or `already`, so a line saying
  while the suite was already running is silent. Two may not: the pair stops
  being a verb group once a noun phrase can sit between the halves. One thing
  to know about that filter, since what it costs you is a report and never a
  wrong one: a concession whose own clause carries a progressive goes
  quiet, as in while the parser was correctly parsing directives, the getter
  was missing. Of 29 such sentences measured, 27 were describing time and 2
  conceded. The rule guard carries the numbers and the narrower rule that was
  built for those 2 and measured worse. The
  participial drop also asks that the clause carry no
  finite verb, from a closed set of twenty forms, because a participial adjunct
  has no subject of its own and a finite verb means the `-ing` word is
  modifying a noun or standing as a subject instead. That keeps lines saying
  while programming language parsers are written manually and while
  manipulating ASTs is the most flexible way reporting as the concessions they
  are. `be`, `been`, and `being` are outside that set on purpose, so a line
  saying while being tested still drops.
- `SLOP-C004`'s staged-agreement arm reads across a block edge, which is a
  sentence boundary and a stronger one than a period, so a concession opening
  the next list item reports. The span opens at the concession word in every
  case, so a finding never carries the terminal period that licensed it.
- `SLOP-C011`'s open hedge forms read a noun phrase of up to three tokens, which
  covers a stacked determiner as in no one single finding. Three is where it
  stops: a fourth token would admit an of-phrase and seat the head-noun test on
  the wrong word.
- `SLOP-C007` no longer reads a participial adjunct as a trailing negation tag.
  Where the negation runs straight into an -ing word, the tail says how someone
  did something, so lines saying she listened, never judging anyone and they
  shipped it quietly, not making a fuss are silent. A determiner in between puts
  a real noun back in the tail, so not the beginning keeps firing, and five
  words that end in -ing without being participles are denied, which keeps not
  everything firing too.

### Fixed

- A document whose first character was not ASCII failed the whole run with an
  instrumentation error and exit 30, as soon as any rule that speaks about the
  document as a whole had something to say. A leading emoji did it, and so did
  a leading em dash or an accented letter. Those rules report the opening of
  the payload as their span, and the span was one byte wide, which lands
  mid-character on anything outside ASCII and trips the check that every
  reported span sits on character boundaries. The span is now the first
  character, however many bytes that takes, and the four rules that report one
  share a single helper. The failure was loud, so no report was ever wrong
  because of it, but a draft opening on an emoji could not be linted at all.

## [0.1.2] - 2026-08-20

### Added

- `SLOP-C011` proleptic-capability-denial, candidate tier, blocking, on in all
  six profiles. It reports a denial of a capability nobody claimed. Two clause
  families qualify. One is a denied capability in three spellings: a subject at
  the head of its clause with a negation and then a verb from the closed
  capability list, a negative subject with the same verb, or the fragment whose
  subject was left out. The other is an evidential hedge from a closed phrase
  list. Two qualifying clauses in one block report on their own. A single clause
  reports beside an affirmative partner that speaks about the same thing, found
  in its own sentence, the one before, or the one after. Two clauses speak about
  the same thing when either uses a bare pronoun, or when both name the same
  tool noun or the same product. Each qualifying clause reports its own span and
  judge question. A clause opening on a negation that
  governs a base-form verb is an instruction to the reader and stays silent.
- `SLOP-F005` rationale-leak, also a blocking candidate everywhere. It reports a
  design argument or a reception instruction left in the text, and it counts a
  marker only where a tool noun stands in the same sentence.
- `SLOP-C007` gains the and-not spelling, the same figure with its second half
  left out. The or and but spellings stay a hand read, and the guard says why.

### Changed

- The README opens on what the linter does and drops the paragraph about
  authorship. The Rust-source paragraph is now three sentences.
- The skill states that the linter and the document run together on every draft,
  carries the two new patterns as entries 32 and 33, and gains three hand-read
  tells, the empty restatement, the dangling which-clause, and the noun pile in
  front of the verb.

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
  language reaches the rules and produces findings that include its punctuation.
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
