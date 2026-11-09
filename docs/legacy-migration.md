# Legacy project data migration

Session startup imports the old project-local `.harness/` data into the
project's central Epic data directory. Directory existence is not a completion
signal: a startup lock or an interrupted import may have created it already.

The import writes `legacy_migration.pending` before copying and only writes
`legacy_migration.complete` after every transferable entry is preserved. A
partial import reports its status and retries on a later startup or compact
event. Identical destination files count as completed transfers; distinct
existing files are never replaced. Resolve such a conflict explicitly before
retrying.

Original legacy data remains in place as an inactive safety backup after a
successful import. New activity uses the central store. Keeping originals also
avoids deleting data written by another legacy process during migration.
Project-local `worktrees/`, `guard-rules.yaml` and `engagement.md` stay in place
and are not imported or removed. The completion marker prevents repeated
imports of the retained backup.

Directory links and Windows reparse points are reported as unsupported rather
than traversed. Failed transfers retain their originals. Copies are verified
before an atomic, non-overwriting destination is installed; a filesystem that
cannot perform that operation produces a partial-import result instead of a
false success.
