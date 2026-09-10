# Fast local delivery

Prefix an implementation request with `!fast` for a local iteration: code plus the smallest useful
debug run only. It skips checks, packages, screenshots, benchmarks, progress documentation,
commits, pushes, and pull requests.

The agent ends with a request to check the result. After an explicit `confirm`, it reviews the
task files, commits them, and pushes `main`. Security and product-boundary requirements still
apply.
