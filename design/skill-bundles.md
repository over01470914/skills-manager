# Skill bundles: first development slice

## Goal

A bundle is a reusable task workflow. A user invokes `$<bundle>` in Codex or `/<bundle>` in Hermes. The agent reads a small entry skill, then asks Skills Manager which stage is ready and reads only the skills needed for that stage. The original skills remain independent library entries and retain their own update sources.

Imported manifests live under the central skills Git repository at `.skills-manager/bundles/<slug>.yaml`, alongside existing Skills Manager metadata, so ordinary Git backup includes them.

## Boundaries

- A **skill** supplies one capability or procedure.
- A **preset** controls deployment of a set of skills to agents.
- A **bundle** controls the order and conditions under which an agent reads skills during one task. Creating a bundle does not deploy its dependencies.
- A **category** is only a UI label; it does not change loading behavior.

The first slice supports direct skill dependencies only. Nested bundles, automatic installation of missing skills, arbitrary expressions, and background execution are out of scope.

## Manifest draft

```yaml
schema_version: 1
slug: web-release
description: Build, review, and release a web page.
instructions: Follow stages in order and report each stage's result.
stages:
  - id: build
    skills: [frontend-design]
  - id: review
    after: [build]
    when: page-available
    skills: [web-design-reviewer]
```

`slug` is unique across bundles and must not collide with an existing target skill or Hermes native bundle. `skills` resolves by stable library identity at save time; the human-readable names above are an import format. `after` references stage IDs. `when` names a Boolean fact supplied by the agent to the resolver; it is not executable code. Validation rejects missing skills, duplicate slugs or stage IDs, cycles, unknown facts, and invalid paths before deployment.

## First CLI surface

- `bundles list` and `bundles show <slug>` inspect definitions.
- `bundles validate <manifest>` checks structure and library references without changing state.
- `bundles import <manifest>` saves a manifest and resolves skill names to stable library IDs.
- `bundles resolve <slug> --stage <id> [--completed build] [--fact page-available=true] --json` returns that stage's instructions and canonical SKILL.md paths. It reads neither prior nor future skill bodies.
- `bundles export-entry <slug> --dest <directory>` creates a small ordinary entry skill for inspection or manual installation.
- `bundles deploy <slug> --agent codex|hermes [--dry-run]` installs that entry as a managed library skill and deploys it through the existing conflict-protected flow. `bundles undeploy` removes only its managed Agent copy.

The entry skill tells the agent to call `resolve` at each stage and read only returned paths. Hermes's native bundle feature is not used for this route because it loads all listed skills at invocation time.

## Acceptance criteria

1. Two bundles can reference the same skill without copying or altering it.
2. Codex `$web-release` and Hermes `/web-release` load the entry skill, then only the `build` dependency initially.
3. `review` is unavailable until `build` completes and `page_available=true` is supplied.
4. A missing dependency, stage cycle, name collision, or unmanaged target yields a specific error with no partial deployment.
5. Windows and macOS resolve paths correctly. CLI JSON output is stable and tests cover the resolver and deployment conflict behavior.
6. Existing presets and skill deployments retain their current behavior.

## Integration order

1. Manifest parser, validator, and resolver in the Rust core, with focused tests.
2. CLI commands and generated entry skill for Codex and Hermes, using existing skill deployment for the entry.
3. Conversation-level acceptance on both agents.
4. Desktop UI editor and Git backup serialization after the runtime behavior is verified.
