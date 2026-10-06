# Desktop maintenance

The `desktop` branch adds paired-client clipboard HTTP requests to the upstream
Moonlight library. Upstream base: `d6fd16f90cd298a081800788f072e457defac1a6`.

`origin` is Ficik/moonlight-common-rust; `upstream` is
MrCreativ3001/moonlight-common-rust. Keep the clipboard implementation and CI in
separate commits. To update, fetch upstream, create `upgrade/<version>` from
`desktop`, and rebase the desktop commits onto the selected upstream revision.
Run CI before updating `desktop`. Never move an existing release tag.

Consumers pin the exact commit, not the moving branch. After a library update,
update both Cargo.toml and ubrn.config.yaml in Ficik/moonlight-web-stream and
regenerate Cargo.lock. Build and test that repository before releasing the stack.
