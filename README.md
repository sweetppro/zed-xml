# Zed XML

XML syntax highlighting for [Zed](https://github.com/zed-industries/zed).

## Adding file types

This extension only claims `.xml` by default, plus any file whose first line matches an XML declaration (`^<.*xml`). There are too many XML based formats to hardcode them all, so extra extensions are left to you.

To associate more file types, add them to your Zed `settings.json`:

```json
{
  "file_types": {
    "XML": ["*.svg", "*.xsl", "*.dtd"]
  }
}
```

## Tree-Sitter

- https://github.com/tree-sitter-grammars/tree-sitter-xml
