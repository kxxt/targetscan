# Changelog

## 0.1.1

- Expand Linux release downloads from 3 to 23 targets. New builds cover 32-bit x86, ARMv6, ARMv7, LoongArch64, PowerPC, RISC-V, s390x, and SPARC64, plus a musl build for ARM64. Each archive has a matching SHA-256 checksum file.

## 0.1.0

- Scan the current directory or a supplied path for conventional Cargo target directories and custom outputs containing Cargo’s compiler-info marker.
- Compare build sizes in a treemap or a largest-first tree with background bars. Both views show human-readable sizes and share a color gradient scaled to the scan.
- Navigate with arrow keys or the mouse wheel, mark multiple targets, and review their paths before confirming permanent deletion.
- Continue browsing during deletion. The refreshed results retain focus and marks for surviving targets.
