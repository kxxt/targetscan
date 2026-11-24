# Changelog

## 0.1.0

- Scan the current directory or a supplied path for conventional Cargo target directories and custom outputs containing Cargo’s compiler-info marker.
- Compare build sizes in a treemap or a largest-first tree with background bars. Both views show human-readable sizes and share a color gradient scaled to the scan.
- Navigate with arrow keys or the mouse wheel, mark multiple targets, and review their paths before confirming permanent deletion.
- Continue browsing during deletion. The refreshed results retain focus and marks for surviving targets.
