# Failed project transfers

`merge-project` returns failure when a database or file transfer fails. Global database changes roll back together. `--delete-source` retains the source directory after any unsuccessful transfer. Database and filesystem domains remain separate; an earlier successful domain may need reconciliation before retrying. Conflicting scalar state fails instead of discarding source state.
