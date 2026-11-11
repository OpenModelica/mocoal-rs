# License

mocoal-rs

Copyright (c) 2026 Open Source Modelica Consortium (OSMC)

It is provided under the terms of the GNU Affero General Public License version 3 (AGPL v3) or
the OSMC Public License (OSMC-PL) version 1.8, according to the recipient's choice:

- **AGPL v3:** the full text is in [COPYING](COPYING), and at
  https://www.gnu.org/licenses/agpl-3.0.html.
- **OSMC-PL 1.8:** the full text is in [OSMC-License.txt](OSMC-License.txt), as distributed with
  OpenModelica (https://github.com/OpenModelica/OpenModelica).

This program is distributed WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE, EXCEPT AS EXPRESSLY SET FORTH IN THE BY
RECIPIENT SELECTED SUBSIDIARY LICENSE CONDITIONS OF OSMC-PL.

## Third-party code

### In this repository

`src/spring.rs` is adapted from `spring_layout` in GraphPlot.jl
(https://github.com/JuliaGraphs/GraphPlot.jl), MIT "Expat" License, Copyright (c) 2015:
afternone.

`sind` and `cosd` in `src/numbers.rs` are written after `base/special/trig.jl` of Julia
(https://github.com/JuliaLang/julia), MIT License, Copyright (c) 2009-2025: Jeff Bezanson,
Stefan Karpinski, Viral B. Shah, and other contributors:
https://github.com/JuliaLang/julia/contributors.

The MIT License of both:

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software
> and associated documentation files (the "Software"), to deal in the Software without
> restriction, including without limitation the rights to use, copy, modify, merge, publish,
> distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
> Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
> BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

The two kernel functions of `sind` and `cosd` come, through Julia, from openlibm (`k_sin.c`,
`k_cos.c`):

> Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
>
> Developed at SunPro, a Sun Microsystems, Inc. business.
> Permission to use, copy, modify, and distribute this
> software is freely granted, provided that this notice
> is preserved.

`examples/` shows six example models of the Modelica Standard Library (MSL) 4.1.0: OMEdit
pictures of their diagrams with the library's layout and with mocoal's, and SVG previews of
mocoal's layout. `examples/Drive.mo` uses MSL classes by name. No MSL files are in this
repository. `test/library/Parts.mo` is written for the tests; its class and connector names, the
placements of its connectors and the triangle icon of `RealInput` and `RealOutput` follow the MSL.

The MSL is licensed by the Modelica Association under the 3-Clause BSD License, Copyright (c)
1998-2025, Modelica Association and contributors (https://github.com/modelica/ModelicaStandardLibrary):

> Redistribution and use in source and binary forms, with or without modification, are permitted
> provided that the following conditions are met:
>
> 1. Redistributions of source code must retain the above copyright notice, this list of
>    conditions and the following disclaimer.
> 2. Redistributions in binary form must reproduce the above copyright notice, this list of
>    conditions and the following disclaimer in the documentation and/or other materials provided
>    with the distribution.
> 3. Neither the name of the copyright holder nor the names of its contributors may be used to
>    endorse or promote products derived from this software without specific prior written
>    permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
> IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
> FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
> CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
> DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
> DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
> IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
> OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

### Linked into the program and the library

Not in this repository; the build fetches them.

- The parser of OpenModelica: the crates `openmodelica_ast`, `openmodelica_error`,
  `openmodelica_wasi`, `metamodelica` and `metamodelica_derive`
  (https://github.com/OpenModelica/OpenModelica, `OMCompiler/Compiler/OpenModelica.rs`), AGPL v3
  or OSMC-PL 1.8. `scripts/fetch-omc.sh` fetches them at the commit in `omc/COMMIT`.
- Crates from crates.io, at the versions in `Cargo.lock`, each under MIT, Apache-2.0 or a licence
  as permissive: `cargo metadata --format-version 1` lists the licence of each.
