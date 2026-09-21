# Release signing

This directory holds the public half of the minisign keypair used to sign
GitHub Release archives.

- `dm-minisign.pub` — public key, embedded in `src/infrastructure/self_update/mod.rs` and
  `scripts/install.sh`.
- The private key is **not committed**; configure it as the GitHub Actions
  secret `MINISIGN_SECRET_KEY` before publishing a release.

Generate a replacement keypair with rsign2:

```sh
rsign generate -p signing/dm-minisign.pub -s /tmp/dm-minisign-secret.key -W --unencrypted
```

Key rotation must update this file, the embedded key in
`src/infrastructure/self_update/mod.rs`, the key in `scripts/install.sh`, and
the `MINISIGN_SECRET_KEY` Actions secret together. Verify a candidate archive
through both `scripts/install.sh` and `dm self-update` before publishing.

Never commit the private key or print it in CI logs. `DM_MINISIGN_PUBLIC_KEY`
is intended for tests and private distribution; it changes the self-update
trust root and should not be set from untrusted input.
