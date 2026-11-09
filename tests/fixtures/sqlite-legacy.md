`sqlite-legacy.db` contains synthetic test data only. It was generated with
Python's SQLite 3.50.4 before the Epic engine update:

```python
import sqlite3
db = sqlite3.connect('sqlite-legacy.db')
db.execute('CREATE TABLE memories (id INTEGER PRIMARY KEY, body TEXT NOT NULL)')
db.execute('INSERT INTO memories(body) VALUES (?)', ('synthetic pre-upgrade memory',))
db.commit()
db.close()
```

The test copies this fixture to a temporary directory, reads the old row, and
writes a new row using Epic's actual SQLx engine. No installed harness data is used.
