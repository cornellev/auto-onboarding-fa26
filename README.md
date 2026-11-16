# CEV freshman autonomy onboarding

## Fork and clone

Open https://github.com/cornellev/auto-onboarding-fa26 and click Fork. Clone that fork:

```sh
git clone https://github.com/YOUR_USERNAME/auto-onboarding-fa26.git
cd auto-onboarding-fa26
git remote add upstream https://github.com/cornellev/auto-onboarding-fa26.git
```

`upstream` is the original repo. When we publish an update, including the simulator this weekend:

```sh
git pull upstream main
```

This is the starter for the month-long minicar project. You'll write how it sees obstacles, how it picks a route, and how it follows that route. The simulator is coming this weekend. Until then, the occupancy grid runs from a saved scan.

Start with [getting started](docs/getting-started.md). The project itself, including what we already wrote for you, is [the writeup](docs/project.md).

The first session is:

```sh
cargo test --locked
cargo run -- perception
```

A green `cargo test --locked` does not mean the three algorithms work. Those checks stay ignored until you ask for them:

```sh
cargo test --test perception -- --ignored
cargo test --test planning -- --ignored
cargo test --test controls -- --ignored
```

`autonomous` is in the writeup, after the three pieces work.
