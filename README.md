# CEV freshman autonomy onboarding

This is the starter for the month-long minicar project. The car already connects to the simulator and stays stopped. You'll write how it sees obstacles, how it picks a route, and how it follows that route.

Start with [getting started](docs/getting-started.md). The project itself, including what we already wrote for you, is [the writeup](docs/project.md).

Once the tools are installed, the first session is:

```sh
cargo test --locked
cargo run -- observe
```

A green `cargo test --locked` does not mean the three algorithms work. Those checks stay ignored until you ask for them:

```sh
cargo test --test perception -- --ignored
cargo test --test planning -- --ignored
cargo test --test controls -- --ignored
```

`autonomous` is in the writeup, after the three pieces work.
