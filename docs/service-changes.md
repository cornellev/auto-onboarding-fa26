# Service implementation handoff

The service changes are isolated on `cev-onboarding-protobuf` branches in both
existing Git repositories. Their original working checkouts, including unrelated
uncommitted work, were preserved. Local review checkouts are kept under the ignored
`services/orchestrator` and `services/sim` directories in this project.

Exact source revisions are in `config/services.json`. The Rust `simcontrol` client
keeps its tested base revision because its lifecycle API did not change. Simulator
and orchestrator service revisions include the new transport and installation work.

Orchestrator is published in [draft PR #1](https://github.com/cornellev/orchestrator/pull/1)
at the pinned revision. The simulator branch remains local, as requested; it has
not been pushed and has no pull request. Use its local checkout for installation
and integration checks. Remote combined installation and transport CI still require
the simulator revision to be distributed.

For immediate local verification, install Python dependencies into a Python 3.12
environment, then run from the onboarding root:

```sh
python -m pip install -r services/orchestrator/requirements.txt -r services/orchestrator/requirements-protobuf.txt
python services/orchestrator/scripts/install_zenoh.py --directory .tools
python scripts/check-stack.py --orchestrator services/orchestrator --sim services/sim --router .tools/zenohd
```

On Windows use `.tools/zenohd.exe`. This check starts isolated loopback services on
unused ports, checks the actual JavaScript/Python/Zenoh/Rust round trip and three
Rust processes, then cleans up its own processes. It does not require a browser.

For browser checks, start the services using `getting-started.md`. Use
`python main.py --zenoh-profile cev-onboarding-v1` from the orchestrator checkout if
its `orch` command has not been installed. Start `npm run dev` from the simulator
checkout. The new simulator defaults to the existing ports; alternate endpoint
environment variables support independent testing without stopping other services.

The combined installer was tested from clean local clones with the exact service
revisions, Node 22.22.2, isolated command paths, and no changes to shell startup files.
Both `orch` and `zenohd` honor `ORCHESTRATOR_BIN_DIR`. The final simulator commit only
improves the printed startup instructions after that successful installation.
