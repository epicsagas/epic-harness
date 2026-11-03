# Sonnet validation v1 (2026-11-03)

12 SWE-bench Verified instances (`../manifest_sonnet_v1.jsonl`: the 4 pilot instances plus 2 per band, seed 20261103), one seed, `claude-sonnet-5-5` through the `native` profile. epic ran the binary built at `16396c2` (includes the polish fix `eb4d20b`). Graded with swebench 4.1.0 on amd64 images under Rosetta.

| Arm | Resolved | Cost (USD) | Turns | Patch bytes |
|---|---|---|---|---|
| bare | 10/12 | 1.53 | 81 | 16,757 |
| epic | 8/12 | 1.61 | 80 | 14,136 |

## What this validates

- The polish fix holds: no whole-file reformat on any of the 12 epic cells. 8 of 12 epic patches are byte-identical in size to bare. The two pilot failures are gone: django-11740 is 1,262 B (pilot: 61,684 B) and sympy-23413 is 1,174 B (pilot: empty), both resolved.

## What it does not show

- The 2 discordant instances (sympy-12489, django-11728, both B4, bare resolved and epic not) are model variance, not harness interference. No formatter or guard activity appears in either transcript. django-11728: epic fixed `replace_named_groups` but not the sibling `replace_unnamed_groups`; its local check was vacuous (admin_docs tests skipped). sympy-12489: epic did not run the test suite and broke 4 PASS_TO_PASS tests.
- 2 versus 0 discordant pairs gives exact McNemar p = 0.5. No resolve-rate difference is supported.
- Contamination: most cells finish in 3 to 10 turns with no exploration, grepping straight to the fixed function. Sonnet appears to recall these Verified fixes, so resolve rate here measures recall more than harness value. A harness comparison needs a less contaminated set (for example SWE-bench Live or private tasks).
