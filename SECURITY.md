# Security

Report vulnerabilities privately through GitHub: **Security → Report a vulnerability** on <https://github.com/drumst0ck/ferrocast>. Please do not open a public issue for them.

Things to know:

- Data is stored as plain JSON in `~/Library/Application Support/Ferrocast` (not encrypted). API keys live in the macOS Keychain.
- Script Commands pass arguments, including `password` arguments, on the command line, as Raycast does, so they are visible with `ps` while the script runs. Do not put critical secrets there.
- Conversions run locally; nothing is uploaded.
