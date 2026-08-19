# Updater fixtures — TEST ONLY

This directory is a local verification harness for Keysmith Switch updater policy.

**Do not use these keys for GitHub Releases, production builds, or GitHub Secrets.**

## What is here

| File | Role |
| --- | --- |
| `TEST_ONLY.minisign.key` | Fixture private key (empty password). TEST ONLY. |
| `TEST_ONLY.minisign.key.pub` | Fixture public key, base64 of a minisign `.pub` file. Copied into `tauri.conf.json`. |
| `TEST_ONLY.wrong.key` / `.pub` | Second pair used to produce a valid-but-wrong signature. |
| `artifact-0.2.0.bin` (+ `.sig`) | Signed upgrade payload for current `0.1.0`. |
| `artifact-0.2.0-beta.1.bin` (+ `.sig`) | Signed beta payload. |
| `artifact-0.0.9.bin` (+ `.sig`) | Signed downgrade payload (policy must reject). |
| `artifact-0.2.0.bin.sig.wrong` | Signature of `0.2.0` by the wrong key. |
| `minisign_verify/` | Vendored MIT `minisign-verify` 0.2.5, used only as a path module. |

Default stable endpoint (runtime, not this fixture):

`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`

Beta:

`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/download/beta-latest/latest.json`

## Environment overrides

```bash
KEYSMITH_SWITCH_UPDATER_ENDPOINT      # full latest.json URL
KEYSMITH_SWITCH_UPDATER_ENDPOINT_BASE # host root; channel path is appended
KEYSMITH_SWITCH_UPDATER_PUBKEY        # tauri-style pubkey string
KEYSMITH_SWITCH_UPDATE_CHANNEL        # stable | beta
```

Never set a GitHub token on the updater client. Metadata is public.

## Regenerate the TEST ONLY pair

From `src-tauri/`:

```bash
npx --yes @tauri-apps/cli signer generate --ci -p "" -w fixtures/updater/TEST_ONLY.minisign.key -f
npx --yes @tauri-apps/cli signer sign -f fixtures/updater/TEST_ONLY.minisign.key -p "" fixtures/updater/artifact-0.2.0.bin
```

Copy `TEST_ONLY.minisign.key.pub` into `tauri.conf.json` `plugins.updater.pubkey` and `updater::FIXTURE_PUBKEY`.

Production signing uses a **different** key injected as `TAURI_SIGNING_PRIVATE_KEY` via GitHub Secrets. That secret is not created in this pass.
