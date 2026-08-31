# Security policy

Security fixes are prioritized for the latest GitHub Release. Older versions might not receive separate backports, so reproduce issues on the latest release when practical.

## Report a vulnerability

Use **Report a vulnerability** in the repository's Security section. Do not create a public issue or upload real credentials, user databases, private paths, or personal logs.

Include the affected Mosaic and Windows versions, minimal reproduction steps, expected impact, and redacted error details. Describe any attachment and its sensitivity before sharing it.

Maintainers will acknowledge the report, assess impact, and coordinate remediation and disclosure.

## In scope

Examples include update signature or hash bypasses, community ZIP path escapes, plaintext credential exposure, cross-task permission bugs, and unsafe process lifecycle behavior.

Malicious behavior inside a third-party script is not a Mosaic sandbox vulnerability because the current release does not claim operating-system isolation. Bypassing installation checks or misleading users about declared risk remains in scope.
