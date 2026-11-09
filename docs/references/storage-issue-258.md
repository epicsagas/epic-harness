# Project-scoped legacy migration

Completion and interrupted markers belong to the current project. `--reset` clears only that project's interrupted state. Repeated successful imports are no-ops. Old releases recorded one global marker without source identity; it cannot establish that other projects were imported. Before explicitly reimporting old already-imported files, inspect retained sources and the database because provenance cannot safely be reconstructed from that marker alone. Ordinary schema initialization does not run this explicit importer.
