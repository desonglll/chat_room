# Server configuration

The server reads `chat-room.toml` from its working directory by default. Use
`--config PATH` to load another file. If the selected file does not exist, the
built-in defaults are used.

```toml
[uploads]
max_file_size_mib = 512

[attachments]
directory = "chat_attachments"
```

`max_file_size_mib` is the maximum size of one file, measured in MiB. It must
be greater than zero. Restart the server after changing it. The browser client
reads the effective value from `GET /api/config`, so its validation and error
messages stay aligned with the server.

`attachments.directory` is the local directory used for attachment bytes. It
must not be empty. Relative paths are resolved from the server working
directory. Uploads are streamed into `.staging`, flushed, and atomically moved
to UUID-sharded paths; SQLite stores attachment metadata only.

When upgrading a database that still has `attachments.data`, startup exports
every BLOB to this directory before migration 10 removes the column. Startup
stops without dropping the column if any export fails. Back up both the SQLite
database and this directory together.
