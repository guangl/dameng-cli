# Release signing

This directory holds the public half of the minisign keypair used to sign
GitHub Release archives.

- `dm-minisign.pub` — public key, embedded in `src/self_update.rs` and
  `scripts/install.sh`.
- The private key is **not committed**; configure it as the GitHub Actions
  secret `MINISIGN_SECRET_KEY` before publishing a release.

Generate a replacement keypair with rsign2:

```sh
rsign generate -p signing/dm-minisign.pub -s /tmp/dm-minisign-secret.key -W --unencrypted
```
