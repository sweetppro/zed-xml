# Development Guide

You must install [zed-extension](https://github.com/zed-industries/zed/tree/main/crates/extension_cli) cli first.

## Build and Test

Run `make build` to build the project, this command will build extension and copy it to the zed-cli extension directory.

```bash
make build
```

After build successfully, your Zed will automatically redload the extension, if no effect, try restart the Zed.

## Release new extension version

1. Update the version in `extension.toml` and `Cargo.toml`.
2. Create a tag with the version number.
3. Push the tag to the repository, then the GitHub Actions will make PR to [Zed extensions](https://github.com/zed-industries/extensions).
4. Wait for the PR to be merged, then the new version will be released.
