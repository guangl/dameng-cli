# dm-plugin-support

Internal utilities shared by the built-in database and SSH plugins. This crate is built inside the repository workspace and is not published.

- `codec`: the hexadecimal format used by saved secrets and exports.
- `secrets`: AES-GCM payloads with a 12-byte nonce prefix; key policy belongs to each plugin.
- `private_file`: create-only, private export files, including cleanup after failed writes.

External plugins should depend on `dm-plugin-sdk`, which defines the public process protocol. Utilities here do not depend on the host, plugin Context, database schemas or terminal prompts.
