# Maintainer Calibration Samples

Sample collection date: July 9, 2026.

This is a dated maintainer calibration record, not a public benchmark or
normative user reference. These anonymized samples are useful for checking
report volume, detector balance, and runtime on large repositories, but they
should not be treated as a default-threshold mandate.

Source identities and local collection paths are intentionally omitted. Raw
reports were generated outside the committed documentation set.

This pass used reproducible static settings for threshold and scoring
sanity-checks:
`--churn off --hotspot-model static --output json --progress never`.

## Commands

| Sample | Command | Status |
| --- | --- | --- |
| large-cli-a | `reforge scan <large-cli-a> --output json --output-file target/calibration/large-cli-a.json --progress never --churn off --hotspot-model static` | Completed. Reported scan duration: 780,923 ms. |
| large-cli-b | `reforge scan <large-cli-b> --output json --output-file target/calibration/large-cli-b.json --progress never --churn off --hotspot-model static` | Completed. Reported scan duration: 122,485 ms. |

## Sample Summary

| Sample | Source files | Directories | Function candidates | Raw functions | Raw types | Test files | Findings | Hotspots | Similar function groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| large-cli-a | 3,205 | 715 | 7,171 | 38,534 | 7,586 | 737 | 4,844 | 3,126 | 69 |
| large-cli-b | 2,178 | 563 | 11,298 | 31,382 | 2,168 | 954 | 3,725 | 1,885 | 4 |

Finding severity:

| Sample | Critical | Warning | Info |
| --- | ---: | ---: | ---: |
| large-cli-a | 0 | 4,177 | 667 |
| large-cli-b | 0 | 3,148 | 577 |

## Top Finding Kinds

| Kind | large-cli-a | large-cli-b |
| --- | ---: | ---: |
| complex_function | 869 | 793 |
| long_function | 706 | 611 |
| readability_risk | 555 | 589 |
| many_parameters | 554 | 86 |
| large_file | 364 | 157 |
| repeated_literal | 292 | 54 |
| deep_nesting | 291 | 390 |
| debt_marker | 261 | 94 |
| import_heavy_file | 184 | 40 |
| test_duplication | 122 | 626 |
| large_type | 65 | 90 |

## Static Hotspot Summary

Raw hotspot locations are omitted from this committed note. The top static
hotspots were retained only as aggregate shape data so the note documents
watchlist behavior without exposing sample-specific paths.

| Sample | Top hotspot priority | Top hotspot severity | File hotspots | Function hotspots | Type hotspots |
| --- | ---: | --- | ---: | ---: | ---: |
| large-cli-a | 100 | critical | 2 | 2 | 1 |
| large-cli-b | 100 | critical | 0 | 4 | 1 |

## Metric Percentiles

| Metric | large-cli-a p50 | large-cli-a p75 | large-cli-a p90 | large-cli-a p95 | large-cli-a max | large-cli-b p50 | large-cli-b p75 | large-cli-b p90 | large-cli-b p95 | large-cli-b max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| file loc | 128 | 392 | 881 | 1,447 | 11,396 | 155 | 326 | 653 | 992 | 5,430 |
| file imports | 6 | 18 | 33 | 47 | 524 | 4 | 7 | 10 | 14 | 109 |
| file public items | 1 | 3 | 9 | 16 | 758 | 1 | 2 | 5 | 8 | 209 |
| function loc | 14 | 29 | 54 | 77 | 2,228 | 8 | 22 | 49 | 90 | 3,911 |
| function complexity | 1 | 3 | 7 | 11 | 444 | 1 | 1 | 5 | 10 | 476 |
| function nesting depth | 0 | 2 | 2 | 3 | 10 | 0 | 0 | 2 | 3 | 15 |
| function parameter count | 1 | 2 | 3 | 4 | 34 | 0 | 1 | 1 | 2 | 19 |
| type loc | 5 | 8 | 14 | 22 | 929 | 7 | 17 | 80 | 194 | 3,427 |
| type member count | 2 | 4 | 7 | 10 | 270 | 3 | 6 | 11 | 16 | 492 |

## Observations

- Both samples were scanned with churn disabled, so hotspot ranking is entirely
  static. This makes the sample reproducible but does not calibrate hybrid
  ranking behavior.
- The samples are large: 1,149,253 and 652,390 total scanned file LOC
  respectively. Debug scans are expensive on this scale.
- Neither sample produced critical findings, but both produced many critical
  static hotspots. This supports keeping hotspots as a watchlist rather than a
  hard CI gate.
- `large_file` default `800` sits near the upper decile for both samples:
  large-cli-a file LOC p90 is 881 and large-cli-b p90 is 653. That threshold is
  broadly plausible for large mixed repositories, but stricter teams may want
  600 or lower.
- `max_function_lines=80` is close to large-cli-a p95 at 77 and below
  large-cli-b p95 at 90. It is a reasonable default, but expect many warnings
  in CLI/TUI orchestration code.
- `max_function_complexity=15` is above p95 for both samples, yet
  `complex_function` is the largest finding kind in both reports. The extreme
  max values suggest the detector is dominated by a long tail rather than by
  normal functions.
- `max_function_parameters=5` behaves differently across samples:
  large-cli-a has 554 `many_parameters` findings while large-cli-b has 86.
  This should stay configurable and should not be weighted as a universal
  style rule.
- Test maintenance signals vary by project shape. large-cli-b has 954 test
  files and 626 `test_duplication` findings, while large-cli-a has 737 test
  files and 122 `test_duplication` findings. Teams may need separate presets
  for production-source gates and test-maintenance audits.
- Similar-function detection produced 69 groups in large-cli-a but only 4 in
  large-cli-b under default thresholds. The current default appears
  conservative for TypeScript-heavy samples but can still be costly on very
  large trees.

## Calibration Follow-Ups

- Add at least one smaller library and one service application before changing
  defaults based on these two large CLI samples.
- Review a stratified sample of high-priority findings with maintainers to
  estimate false-positive rates by detector kind.
- Consider documenting presets rather than changing global defaults: for
  example `balanced`, `strict`, `legacy`, and `production-only`.
- For large repositories, consider a calibration mode or docs recipe that uses
  `--exclude-tests` and higher `--min-function-tokens` when the goal is
  structural threshold calibration rather than duplication analysis.
