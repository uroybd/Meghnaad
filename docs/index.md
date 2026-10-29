# Meghnaad

[Taskwarrior 3](https://taskwarrior.org) in your browser and on your phone. The Rust engine runs in the page (and
natively in the Android app), a small Worker on Cloudflare keeps the secret, your existing S3 or R2 bucket is the sync
store, and Cloudflare Access is the login. Behaviour follows Taskwarrior **3.5.0**, command for command.

![The Tasks view: the next report as a table, with the prompt underneath](screenshots/tasks.png)

## Where to start

| | |
| --- | --- |
| **[Using Meghnaad](./using.md)** | Pages, console, confirmations, boards, hooks, recurrence, urgency, time tracking |
| **[Deploying](./deploy.md)** | Setup, GitHub Actions deploy, Access, connecting `task`, troubleshooting |
| **[The Android app](./android.md)** | Install, connect, the widgets, sharing settings and hooks, building it |
| **[taskrc support](./taskrc-support.md)** | Every `taskrc` option: done, partial, not done |
| **[Architecture](./architecture.md)** | The engine, sync, platform limits, how correctness is kept |

The source, issues and releases are on [GitHub](https://github.com/uroybd/Meghnaad).
