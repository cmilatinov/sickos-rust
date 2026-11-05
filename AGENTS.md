# Repository guidance

Use `agentic:agentic-cli` for Git and GitHub work, and
`agentic:verification-workflow` before committing or opening a PR.
`agentic.toml` declares the checks and the SickOS Roadmap Project.

Kernel execution and tests run in QEMU through the Cargo aliases in
`.cargo/config.toml`. Host builds exist for IDE analysis; exclude `kernel`
when running host tests. Set `SICKOS_HEADLESS=1` for unattended validation.
The pinned Rust toolchain, QEMU with edk2 firmware, and `curl` must be installed.

## Docs

When you need to understand an area, or doubt how or why it works, look
for its doc here first. Update a doc your change contradicts in the same
change.

- [README.md](README.md): setup, build and test commands, Agentic, and QEMU debugging.
- [PLAN.md](PLAN.md): architecture decisions, crate policy, and the phased roadmap.
