---
name: Agent task
about: A self-contained brief a contributor's coding agent can pick up and finish
title: "<verb> <what>"
labels: ["agent-task"]
---

<!-- Briefs are read by coding agents first. Keep every section; write
"none" where one doesn't apply. Rules for every task: AGENTS.md. -->

## Context

<!-- Why this matters and what exists today, in a few sentences. Link the
backlog item, plan section or spec it comes from. -->

## Files

<!-- Where the work happens (paths, functions), and the tests that guard it. -->

## Definition of done

- [ ] <!-- the behaviour, stated so a test can check it -->
- [ ] A scenario test in `<crate>/tests/it/<file>.rs` (or unit tests) covering it
- [ ] The affected crates' fast tier and `cargo test -p repo_checks` pass
      (Linux: `.githooks/pre-commit --worktree` runs exactly that and the
      commit reuses its pass; Windows: `.\scripts\affected-tests.ps1 -Base origin/main`)
- [ ] Docs kept true (`//!` docs; `cargo run -q -p repo_checks --bin docs_index` if one changed)
- [ ] <!-- screenshots before/after, if anything visible changes -->

## Constraints

<!-- Parity rules, data-driven, ECS style, flags (behaviour/order changes to
check), performance limits. -->

## Out of bounds

<!-- What not to touch or decide in this task. -->

## Size and labels

- Size: <!-- S (hours) / M (a day or two) / L (split it) -->
- Area: <!-- area:engine, area:net, area:ui, area:render, area:tools, area:cs_source, area:party -->
- Needs install: <!-- no / cs_source / combat_arms -->
- <!-- good-first-task if a newcomer's agent can do it in one session -->
