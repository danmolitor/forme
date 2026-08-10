# @formepdf/shared

Framework-agnostic core shared by Forme's authoring adapters (`@formepdf/react`, and future adapters): the Forme document-model types, the `Style` type and its mapping to engine JSON (including CSS string shorthands), color/dimension/edge/corner parsing, custom font registration and merging, and the `<Canvas>` operation recorder.

Plain data in → document model out. No framework dependencies.

You normally don't install this package directly — it comes in as a dependency of an adapter, which re-exports everything user-facing.
