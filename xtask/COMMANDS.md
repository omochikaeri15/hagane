# Commands

### cargo ship
Cuts a release: stamps the version, runs `cargo check`, marks which front ends changed, merges `dev` into `main` and pushes.

### cargo ship all
Same as `cargo ship`, but rebuilds every front end.

### cargo ship recover
Retries a release that failed before publishing, under the same version number, with whatever has been committed since. Refuses if that version is already out.

### cargo debug android
Builds the test APK, `compiled/Hagane-dev.apk`, which installs beside the stable app.

### cargo debug apk
Same as `cargo debug android`.

### cargo deploy website
Builds the site for release, replaces any running deploy and serves it at `http://localhost:8080`.

### cargo deploy web
Same as `cargo deploy website`.

### cargo deploy stop
Stops the running deploy.
