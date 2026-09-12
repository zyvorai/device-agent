# Tutorials

Narrative, step-by-step walkthroughs for Zyvor Device Agent. Read them in
order the first time through; after that, jump straight to whichever one
matches what you're doing.

Looking for a fact instead of a walkthrough (an API field, a config key, a
protocol detail)? See the reference docs linked from the main
[README's Documentation section](../../README.md#documentation) instead —
these guides link out to them wherever the two overlap, on purpose, rather
than repeating them.

| # | Guide | You'll come away able to... |
|---|---|---|
| 1 | [Getting started](01-getting-started.md) | Build the agent and dashboard from source and see live hardware inventory on screen |
| 2 | [Configuration & the API](02-configuration-and-api.md) | Read/edit `device-agent.toml` with confidence and call the REST API directly |
| 3 | [Securing your agent](03-securing-your-agent.md) | Pick the right auth mode (none / bearer / mTLS / Unix socket) for your deployment |
| 4 | [Industrial buses](04-industrial-buses.md) | Monitor CAN health, declare RS485 ports, and turn on read-only CAN capture |
| 5 | [Writing a sensor plugin](05-writing-a-sensor-plugin.md) | Write, register and sample a new sensor plugin end to end |
| 6 | [Deploying to production](06-deploying-to-production.md) | Ship a package or remote deployment, reload config live, and scrape metrics |
