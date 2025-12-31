# Zed XML

XML syntax highlighting for [Zed](https://github.com/zed-industries/zed).

See the official [Zed XML Language Documentation](https://zed.dev/docs/languages/xml).

## Tree-Sitter

- https://github.com/tree-sitter-grammars/tree-sitter-xml

## Language Server

This extension uses the [eclipse-lemminx/lemminx](https://github.com/eclipse-lemminx/lemminx) language server.  If a `lemminx` binary is found in your PATH that will be used, otherwise the extension will automatically download pre-built `lemminx` binaries for your platform from [redhat-developer/vscode-xml/releases](https://github.com/redhat-developer/vscode-xml/releases).

## Formatter

### Prettier

Use [prettier](https://prettier.io/) with [prettier/plugin-xml](https://github.com/prettier/plugin-xml) to format:

```json
  "languages": {
    "XML": {
      "prettier": {
        "allowed": true,
        "plugins": ["@prettier/plugin-xml"]
      },
      "formatter": "prettier",
    }
  }
```

### Language Server

Use `lemminx` language server to format documents:

```json
  "languages": {
    "XML": {
      "format_on_save": "on",
      "formatter": "language_server"
    }
  }
```

### Manual formatting

If you prefer, you can manually use `editor::FormatDocument` (`ctrl-shift-i` on Linux, `cmd-shift-i` on macOS) and disable automatic "Format on Save" with:

```json
  "languages": {
    "XML": {
      "format_on_save": "off"
    }
  }
```
