# Third-Party Notices

PCBServer, a fork of Konnect (see NOTICE), incorporates data, algorithms, and
code ported from the following projects. PCBServer is licensed under the
AGPL-3.0-only (see LICENSE); the notices below travel with every distribution
of it.

## KiCad IPC API definitions

`crates/konnect-ipc/proto/` holds unmodified copies of the protocol buffer
definitions from KiCad's `api/proto/` directory
(https://gitlab.com/kicad/code/kicad), compiled into the IPC client. The tag
and commit they were taken from are recorded in
`crates/konnect-ipc/proto/README.md`. Copyright Jon Evans and the KiCad
Developers. Each file keeps its upstream header: all are licensed under the GNU
General Public License, version 3 or (at your option) any later version, except
`common/types/variants.proto` and `common/commands/variant_commands.proto`,
which are licensed under the MIT license whose text is reproduced in those
files. Section 13 of the GPL-3.0 and of the AGPL-3.0 permits combining the two.

## topola_specctra

Konnect uses the `topola_specctra` parser, data model, and serializer from the
[Topola project](https://codeberg.org/topola/topola) to validate and emit
Specctra DSN syntax. Copyright 2024 Topola contributors; used under the MIT
license. The MIT permission grant and warranty disclaimer reproduced below
apply to this dependency.

## kicad-mcp-pro

Portions of Konnect — including the JLCPCB pick-and-place rotation
correction table (itself community-derived; see also yaqwsx/jlcparts and the
KiCad forum), placement scoring weights and heuristics, engineering
calculator formulas and their reference test vectors, and the release-gate
status lattice — are ported from or based on kicad-mcp-pro by Osman Aslan
(https://github.com/oaslananka/kicad-mcp-pro), used under the MIT license:

```
MIT License

Copyright (c) 2026 Osman Aslan

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
