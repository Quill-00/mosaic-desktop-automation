# Mosaic Community Registry

A Mosaic community source is a static JSON registry served over HTTPS. Registries and packages can be hosted directly on GitHub without a central account, login, or download server.

Official source:

```text
https://raw.githubusercontent.com/Quill-00/mosaic-desktop-automation/main/community/registry.json
```

The official source currently contains two auditable MIT examples: offline `Hello Mosaic` and a keyless Open-Meteo weather card. Installed tasks remain disabled until the user enables them.

## Layout

```text
community/
├─ registry.json
├─ registry.schema.json
├─ package.schema.json
├─ Build-Packages.ps1
├─ examples/<package-id>/
│  ├─ mosaic-package.json
│  └─ <entry file>
└─ packages/<package-id>-<version>.zip
```

Every ZIP must contain `mosaic-package.json` directly at its root. Supported runtimes are `node`, `python`, `powerShell`, and `executable`.

## Publish a package

1. Add the smallest auditable source under `community/examples/<package-id>/`.
2. Confirm it contains no `.env`, cookie, token, account, private key, personal path, database, log, output, or chat history.
3. Increment the package version. Published ZIP bytes are immutable.
4. Run `powershell -NoProfile -ExecutionPolicy Bypass -File .\community\Build-Packages.ps1`.
5. Put the generated SHA-256 in `registry.json` and declare the minimum permissions.
6. Validate both schemas, inspect the ZIP file list, and run the example.
7. Submit a pull request. Report security issues privately through `SECURITY.md`.

`packageUrl` must be relative or use an HTTPS URL with the same origin as the registry. Mosaic rejects cross-origin downloads, plaintext HTTP, hash mismatches, traversal, manifest mismatches, and size-limit violations.

## Permissions

Each entry declares `readPaths`, `writePaths`, `allowHosts`, and `channels`. These declarations communicate risk; they are not an operating-system sandbox. Maintainers and users must still review source code.

Third-party maintainers may host compatible registries. Adding a source means trusting its future registry and packages. Prefer immutable release assets, preserve source, pin versions, and review permissions and hashes for every update.
