/* Deterministic regression for CVE-2026-119375. No crafted files or real data.
 * Build this translation unit so we exercise the actual bundled WAL reader.
 */
#include "../vendor/libsqlite3-sys-0.30.1/sqlite3/sqlite3.c"
#include <stdio.h>

static int synthetic_read(sqlite3_file *file, void *output, int count,
                          sqlite3_int64 offset) {
  (void)file;
  (void)offset;
  memset(output, 0x42, count);
  return SQLITE_OK;
}

int main(void) {
  sqlite3_io_methods methods = {0};
  sqlite3_file file = {0};
  Wal wal = {0};
  unsigned char page[4096];
  methods.iVersion = 1;
  methods.xRead = synthetic_read;
  file.pMethods = &methods;
  wal.pWalFd = &file;
  wal.hdr.szPage = 512;
  memset(page, 0xa5, sizeof(page));
  if (sqlite3WalReadFrame(&wal, 1, sizeof(page), page) != SQLITE_OK) return 1;
  for (int i = 0; i < 4096; i++) {
    if (page[i] != (i < 512 ? 0x42 : 0)) {
      fprintf(stderr, "FAIL: WAL read left byte %d uninitialized (%02x)\n", i, page[i]);
      return 1;
    }
  }
  puts("PASS: smaller WAL frames fully initialize the database page");
  return 0;
}
