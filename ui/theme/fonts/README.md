# Vendored font assets

These binaries are embedded into the compiled application by
`ui/theme/typography.slint`, so ZenOmen renders identically on a machine with no
fonts installed and keeps tabular figures aligned in numeric readouts.

| File | Family | Weight | Upstream |
|------|--------|--------|----------|
| `Inter-Regular.ttf` | Inter | 400 | https://github.com/rsms/inter — release `v4.1`, `extras/ttf/Inter-Regular.ttf` |
| `Inter-SemiBold.ttf` | Inter | 600 | https://github.com/rsms/inter — release `v4.1`, `extras/ttf/Inter-SemiBold.ttf` |
| `JetBrainsMono-Regular.ttf` | JetBrains Mono | 400 | https://github.com/JetBrains/JetBrainsMono — `fonts/ttf/JetBrainsMono-Regular.ttf` |

Both families are licensed under the SIL Open Font License 1.1, which permits
bundling with a GPL-3.0 application. The licence texts are kept next to the
binaries: `Inter-OFL.txt` and `JetBrainsMono-OFL.txt`.

Do not substitute a system font for these files: `01-UI-SPEC.md` requires the
exact families at the declared weights, and a silent fallback changes glyph
metrics and numeric alignment.
