---
title: Signature Help
description: Inspect target module parameters, ports, and the current connection position inside instance lists.
---

Signature help appears in instance parameter assignment lists and port connection lists. It is different from completion: completion inserts candidates at the current position, while signature help shows which parameter or port you are currently filling in.

Related settings reference: [Signature Help](../../../advanced-guide/vscode-settings/#signature-help).

## When It Appears

- In parameter assignment lists `#(...)`, it shows target module parameters.
- In port connection lists `(...)`, it shows target module ports.
- As the cursor moves across parameter or port positions, the active item updates.

## Signature Help Is Missing

Signature help needs the target module to resolve. If the instance module name itself does not jump to definition, first check project configuration, include directories, macro definitions, and library dependencies in [Navigation and Reading](../navigation/#definition-does-not-jump).

Enable `vide.signature.help.params.only` if you only want parameter-related signature help.
