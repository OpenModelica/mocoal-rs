# Test models

The test `layouts` in [src/tests.rs](../src/tests.rs) lays out the models in `models/` with the
parts in `library/` and compares each result with its file in `expected/`.

| File | Origin |
|---|---|
| `library/Parts.mo` | Written for these tests. Names, connector placements and the `RealInput`/`RealOutput` icon follow the Modelica Standard Library 4.1.0 (3-Clause BSD License, Modelica Association; see [LICENSE.md](../LICENSE.md)). No MSL classes are copied. |
| `models/Chain.mo`, `Control.mo`, `DriveTrain.mo`, `Features.mo`, `GainChain.mo`, `Inheritance.mo`, `RCLadder.mo`, `ResistorMesh.mo` | Written for these tests. |
| `models/Tickets.mo` | Written for these tests, after the OpenModelica issues on auto layout: `System` is the example of [#16959](https://github.com/OpenModelica/OpenModelica/issues/16959); `Partial` and `Bad` are the missing, partial and bad annotations of [#7232](https://github.com/OpenModelica/OpenModelica/issues/7232). The media and fluid parts are stand-ins, not the MSL's. |
| `expected/*.mo` | Written by mocoal (`MOCOAL_BLESS=1 cargo test -p mocoal layouts`). |
