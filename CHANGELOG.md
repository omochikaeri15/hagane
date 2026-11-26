Changes:
- Bot now accepts Em Dashes and En Dashes for flags.
- Removed many arbitrary rules on the animation frame counter for viewers.
- Added "Soul" animation type for units with unique souls.
- Attack animations now start on frame 1 instead of 0 because frame 0 is unreachable from normal play.
- Now replictes how the source defines a "hit".

Bug Fixes:
- Fixed a bug that caused some non-generic animations to be treated generically under studio and the animation viewer.
- Fixed an off-by-one error with the export loop finder.
- Fixed a bug that caused one specific units attack cycle to be incorrect.
