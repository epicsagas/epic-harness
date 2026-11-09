# CI evidence for Orbit and Ship

`gh pr checks` lists check contexts; an empty list does not establish that a
repository has no CI. GitHub can create workflow runs and check suites requiring
maintainer approval without creating any executable check context.

Both skills first obtain the PR's current `headRefOid` and base repository from
the PR URL. They inspect workflow runs and check suites for that exact SHA,
following pagination where necessary. Results for an older head are discarded.

| Evidence | Outcome | Watch/report behavior |
|---|---|---|
| Same-head run/suite concludes `action_required` | Approval required; pending | Record run links and head; skip the impossible watch; not ready to merge |
| Same-head runs/suites exist without check contexts | Actual pending, success, failure or infrastructure-error state | Inspect run/job metadata; poll unfinished runs only within a declared deadline; verify the current head and successful required jobs before accepting success |
| Executable current-head contexts exist | Pending, pass or fail according to actual results | Watch existing checks; distinguish executed failures from startup errors |
| Successful queries return no contexts, runs or suites | No checks observed | Skip an empty watch; at most one bounded re-probe; do not infer absent or green CI |
| A metadata query fails | Unknown | Preserve the error; do not substitute an empty list |
| No applicable provider is separately verified | N/A | Report the verification; it is not a passing test result |

In the observed approval case, a PR's rollup was empty while its exact head had
CI and Release runs concluding `action_required`. The former Orbit instructions
would incorrectly record “No CI configured.” Ship previously started a watch
without this distinction. The updated guidance preserves the genuine no-CI
escape from an indefinite wait while accurately recording approval and unknown
states.

Orbit still performs post-PR reflection regardless of CI outcome. Additional
reflection context and successful-pattern memory require verified passing CI;
an approval blocker, missing evidence or API error cannot satisfy that condition.
