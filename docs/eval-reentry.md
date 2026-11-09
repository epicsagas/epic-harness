# Eval wrappers and nested evaluation

Use actual measurement commands in `eval.yaml`. A Make/Just target whose job is
only to call `epic eval` is an evaluator entry point, so `epic eval --init` does
not register that direct wrapper as its own benchmark.

Discovery is best effort. Script recipes, context changes and opaque commands
remain eligible for discovery; the runtime check handles indirect recursion.
An inner Eval rejects a repeated canonical project-and-configuration context
with a diagnostic and nonzero exit status. Subdirectories and linked worktrees
share their Git common-directory identity; non-Git projects use canonical cwd.
Initialization and scaffolding remain available without starting evaluation workloads.

Independent projects and distinct evaluation configurations may still nest.
Workload children inherit internal `EPIC_EVAL_CONTEXT_CHAIN` metadata; Epic does
not mutate the invoking process environment or use a global “Eval is running”
lock. Ordinary command output and exit handling remain unchanged.

Existing self-wrapper configs should replace `make eval`/`just eval` entries with
their real benchmark commands. Removing the wrapper is not a measurement; choose
a command whose output/exit status actually evaluates the intended behavior.
