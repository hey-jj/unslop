# unslop

A deterministic linter for formulaic writing patterns: filler, hedging, puffery,
stock transitions, and the punctuation habits that travel with them.
Point it at an essay, a blog post, an email, a report, documentation, or a short
post. It reports the exact span that matched, the rule that matched it, and the
question a person has to answer about it.

A Rust source file is rejected with exit 40. Pass the prose instead, or wrap the
code in a fenced block. unslop lints source in other languages as prose, so its
punctuation shows up in the findings.

## Install

```
cargo install unslop
```

## Use

```
unslop check --profile essay --output text draft.md
unslop check --profile email message.txt
```

`--profile` is required and has no default. `--output text` is the human report and
`--output json` is the machine one, which is the default.

| Exit | Meaning |
|---|---|
| 0 | completed with no unwaived blocking finding |
| 2 | usage error |
| 10 | violations, or a failed verify |
| 20 | unresolved blocking candidates |
| 30 | instrumentation error, fail closed |
| 40 | unsupported input, fail closed |

## Profiles

| Profile | For |
|---|---|
| `essay` | Argued and personal writing, where first person and opinion are the content |
| `blog-post` | Published articles, with presentation rules at full strength |
| `email` | Correspondence, where the chat-assistant register is strictest |
| `report` | Findings someone acts on, where attribution and hedging fire hardest |
| `doc` | Reference and instructional writing, the strictest plain-speech profile |
| `social-post` | Short public posts, with length and structure rules off |

## What it looks for

Ninety-three rules in twenty-two families. Ornamental and promotional vocabulary, puffery,
filler and transition tics, intensifiers and unquantified claims, contrast rhetoric,
and stock attribution. Dash and colon habits, title case, boldface, and emoji. The
chat-assistant register, verbatim self-duplication, and passive voice with the actor
still in the sentence. Some rules only see the document whole, like paragraph
uniformity and the contrast rate per 1000 words.

Each rule states what it matches and why. Each rule that needs a human decision ships
the question with the finding.

Findings are three kinds. A violation is mechanical and blocking. A candidate carries a
judge question and blocks until someone answers it. A coverage hint is instrumentation
and never gates.

## The skill

`skills/unslop/` holds an agent skill that runs the whole loop. Blind read first, then
pick the profile, run the check, adjudicate each finding, revise, and re-run until it
exits 0. Then read once more for the tells no rule catches. The skill carries the full
pattern list with fix guidance and a generated rule reference. The waiver and approval
paths are documented there.

## Two sibling tools

ai-slop gates the prose a code repository ships, including its commits, changelogs, and
package metadata. slop-detector reads text someone else sent you. unslop gates ordinary
writing before it goes out.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
