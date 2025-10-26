# v0.3.0
- Refactor main logic to traits instead of defining via macros, to be more consistent with usual Rust conventions and more flexible for building custom entities
- This version contains major breaking changes for previous model

# v0.2.2
- Fixed VectorSink seemingly leading to poor performance due to scheduling events when not needed based on next process time
- Fixed bug in DelayModeChange returned from DelayModes