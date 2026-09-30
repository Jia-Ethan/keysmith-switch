# Extension source key

`OFFICIAL_PUBKEY.txt` holds the public key of the official extension source, in the
format `tauri signer generate` prints (base64). It is compiled into the app.

While it is empty the official source counts as **not configured**: the Extensions page
says so and never touches the network. Fill it in once the signing key exists (see the
extensions repository's README), then release.

This key must not be the app updater's key. Tests use `fixtures/extensions/TEST_ONLY.key.pub`
instead and never rely on this file.
