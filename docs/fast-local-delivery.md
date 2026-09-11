# Fast local delivery

Prefix an implementation request with `!fast` for a local iteration: code plus the smallest useful
debug run only. It skips checks, packages, screenshots, benchmarks, progress documentation,
commits, pushes, and pull requests.

The agent ends with a request to check the result. After an explicit `confirm`, it checks
workspace and fuzz formatting and runs strict workspace Clippy, using the commands in
`AGENTS.md`. It fixes formatting/lint failures and reruns the checks before committing or
pushing. Blocked or failing checks prevent the push. It then reviews and commits the task
files and required formatting/lint fixes and pushes `main`, preserving unrelated uncommitted
work. Security and product-boundary requirements still apply.
