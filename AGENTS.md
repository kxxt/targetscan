# Guidelines for Agents

## Requirement

- If the user asks you to create a pull request, please include a file named `.agent-created-pr` in it.
- Make sure `cargo clippy --workspace` and `cargo +nightly fmt --all --check` check pass.

## Development Guideline

- Use rust idioms.
- Use [context generic programming](https://contextgeneric.dev/docs/ai/skills/cgp/SKILL/) pattern when suitable. (Install the relevant skill if it is missing)
- Use `clap`/`palc` for command-line parsing.
- Use `serde` for serialization.
- Use `snafu` for error handling within library boundary.
    - Avoid blanket error enum case.
    - The error should carry enough context to debug it. (e.g. Don't say just `Permission denied`. Let's say `Failed to do XXX to XXX: Permission denied`)
- Use `color-eyre` for error handling on application boundary.
- Use `spd-log` for logging and keep a good logging habit.
    - Log important events.
- When writing low-level system operations, look for crates that did it first. E.g. before resorting to `libc` crate, try `nix` or `rustix` first.
- Design for a good user experience.
- Minimize `unwrap` and `expect` usage.
- Avoid `unsafe` code unless necessary.
- Do ablation study on your diffs.
    - Drop unnecessary changes but never remove security relevant checks just to make the diff look smaller.
- Add tests that really matters. We use test-driven design.
    - Tests should be added first and in a separate commit before actually implementing the feature.
    - The code must still compile but the test should fail.
    - Use `test-that` crate for declarative assertions that can produce human readable assertion messages.
- Document invariants that are **not obvious** in comments.
- Do not use the docs as your daybook.
- Mind the crate size. If compilation is painful to wait, perhaps we should split a crate.
    - But never split a crate without a good split of the functionality of it. A lot of refactor work maybe needed and you should pursue it.
- We use GitHub Actions for CI/CD so please maintain them. Use pinned versions with the tag commented at the end.
- When a dependency exposes a feature/environment variable that changes how system libraries are linked,
  expose that feature on our side or document the environment variable in the install docs.
  Common libraries should be linked from the system by default. Rare libraries should be vendored by default.
- Prefer doing things in process to spawning an external command or even shell.
- If an operation may take some time, add a progress bar (e.g. `indicatf` crate).
- Use toml for configuration files.
- For TUI, use `tui-realm`.
