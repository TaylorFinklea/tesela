# Release History

This file tracks all releases of Tesela.

## Release Strategy

Tesela uses a date-based versioning strategy to facilitate frequent and small releases:
- Format: `v{YYYY}.{MM}.{DD}[.{build_number}]`
- Example: `v2024.01.15` (first release of the day) or `v2024.01.15.1` (second release of the day)
- Releases are automatically created on every commit to the main branch
- Each release includes pre-built binaries for Linux, macOS, and Windows

## Installation

### Latest Release

You can always get the latest release from the [releases page](https://github.com/your-username/tesela/releases/latest).

### Quick Install (Linux/macOS)

```bash
# Download latest release (replace VERSION with actual version)
curl -L https://github.com/your-username/tesela/releases/latest/download/tesela-$(uname -s | tr '[:upper:]' '[:lower:]')-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

### From Source

```bash
cargo install --git https://github.com/your-username/tesela
```

---

<!-- Releases will be automatically added below this line -->