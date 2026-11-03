# OSS low-contamination run v1 (2026-11-03)

17 tasks from `../oss/tasks_v1.jsonl`: fix commits from 6 public epicsagas repos, dated 2026-08 to 2026-11, after the model's training data. Each task runs on a history-free snapshot of the base commit (`SNAPSHOT=1`). Hidden tests are the tests the fix commit added; they compile and fail at base and pass at the fix (gold patches grade 17/17). One seed, `claude-sonnet-5-5` through the `native` profile; epic ran the binary built at `16396c2` (includes the polish fix `eb4d20b`). Graded with `../oss/grade.py`.

| Arm | Resolved | Cost (USD) | Patch bytes |
|---|---|---|---|
| bare | 12/17 | 2.22 | 26,008 |
| epic | 12/17 | 2.81 | 26,948 |

- Unresolved by both: epic-harness 691e9dbd, b387ff80; llm-transpile ca68e21f; llm-kernel ddb35602.
- Discordant, one each way: AgentGlance bd0564e9 (epic only; bare missed the environment-variable fallback path) and llm-kernel 1d221fc8 (bare only; epic's span fix still split an escape pair). Exact McNemar p = 1.0.
- Unlike the SWE-bench Verified run, cells explore before editing (typically 4 to 14 turns, 1 to 8 minutes), consistent with the tasks not being recalled.
- epic cost 27% more for the same resolve rate. See the cost breakdown below: most of it is a runner confound, not epic.

## Cost breakdown

Output tokens are about equal (30.2k bare, 31.7k epic) and epic made fewer tool calls (63 versus 76). The gap is the per-turn context prefix, re-read from cache every turn: the first-turn prefix median is 23.6k tokens for bare and 33.7k for epic. From the session transcripts, the extra ~10k tokens per cell split as:

| Source | Tokens per cell | Share | Attributable to |
|---|---|---|---|
| The repo's own `AGENTS.md` / `CLAUDE.md` | ~6.4k | ~2/3 | the runner: `run_swebench.sh` deletes these files only in the bare arm |
| epic plugin skill listing | ~2.7k | ~1/4 | epic |
| `resume` hook additional context | ~0.6k | small | epic |

epic's own overhead is therefore about 3.3k tokens of prefix per session, roughly 9% of cost on these tasks. The runner confound did not exist on SWE-bench (those repos have no agent instruction files). Future runs should keep the repo's instruction files in both arms and strip only harness artifacts (`.harness`, `.claude`, `.agents`).

## Conclusion

On tasks the model cannot recall, epic and bare resolve the same number of tasks with one discordant pair each way. This run supports no resolve-rate effect from epic in either direction; 17 tasks and one seed detect only large effects.

## Run notes

- 6 tasks (12 cells) first hit the 5-hour subscription limit and were rerun after the reset; the merged predictions use the rerun cells.
- Grader fixes made during this run: restore the trailing newline that `$(git diff)` strips from captured patches, and apply the hidden test patch per file so a model that already wrote an identical test hunk (bare on llm-kernel 1487fae6, against the prompt's instruction) is not counted as a conflict.
